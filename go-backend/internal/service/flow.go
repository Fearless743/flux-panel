package service

import (
	"encoding/json"
	"log/slog"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/crypto"
	"github.com/Fearless743/flux-panel/go-backend/internal/gost"
	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/ws"
	"github.com/jmoiron/sqlx"
)

const bytesToGB = 1024 * 1024 * 1024

// FlowService 流量上报与节点配置对账
type FlowService struct {
	DB  *sqlx.DB
	Hub *ws.Hub

	forwardLocks sync.Map // map[int64]*sync.Mutex
	userLocks    sync.Map
	tunnelLocks  sync.Map
}

type FlowDto struct {
	N string `json:"n"`
	U int64  `json:"u"`
	D int64  `json:"d"`
}

type ConfigItem struct {
	Name string `json:"name"`
}

type GostConfigDto struct {
	Limiters []ConfigItem `json:"limiters"`
	Chains   []ConfigItem `json:"chains"`
	Services []ConfigItem `json:"services"`
}

type encryptedMessage struct {
	Encrypted bool   `json:"encrypted"`
	Data      string `json:"data"`
	Timestamp int64  `json:"timestamp"`
}

func NewFlowService(db *sqlx.DB, hub *ws.Hub) *FlowService {
	return &FlowService{DB: db, Hub: hub}
}

// DecryptIfNeeded 解密节点上报 body；非加密则原样返回
func DecryptIfNeeded(raw []byte, secret string) ([]byte, error) {
	if len(raw) == 0 {
		return raw, nil
	}
	var wrap encryptedMessage
	if err := json.Unmarshal(raw, &wrap); err != nil {
		return raw, nil
	}
	if !wrap.Encrypted || wrap.Data == "" {
		return raw, nil
	}
	c := crypto.GetOrCreate(secret)
	if c == nil {
		return raw, nil
	}
	return c.Decrypt(wrap.Data)
}

func (s *FlowService) FindNodeBySecret(secret string) (*model.Node, error) {
	var n model.Node
	err := s.DB.Get(&n, `SELECT * FROM node WHERE secret = ? LIMIT 1`, secret)
	if err != nil {
		return nil, err
	}
	return &n, nil
}

func (s *FlowService) IsValidNode(secret string) bool {
	var cnt int
	_ = s.DB.Get(&cnt, `SELECT COUNT(1) FROM node WHERE secret = ?`, secret)
	return cnt > 0
}

// Upload 处理流量上报数组：同批按解析结果聚合，转发/隧道一次查回算倍率，
// 同一转发/用户/权限的流量合并为一次原子更新，超限检测每类只做一次。
// 与逐条路径最终 DB 状态一致（更新均为原子 +=，暂停幂等且只处理运行中）。
func (s *FlowService) Upload(items []FlowDto) {
	type parsed struct {
		forwardID, userID, utID int64
		d, u                    int64
	}
	parsedItems := make([]parsed, 0, len(items))
	fwdSet := make(map[int64]struct{})
	for _, item := range items {
		if item.N == "" || item.N == "web_api" {
			continue
		}
		f, u, ut, ok := parseFlowName(item.N)
		if !ok {
			continue
		}
		parsedItems = append(parsedItems, parsed{forwardID: f, userID: u, utID: ut, d: item.D, u: item.U})
		fwdSet[f] = struct{}{}
	}
	if len(parsedItems) == 0 {
		return
	}
	// 批量查转发所属隧道与隧道倍率（查不到时倍率为 1，与逐条路径一致）
	tunnelOfForward := make(map[int64]int64)
	if len(fwdSet) > 0 {
		fwdIDs := make([]int64, 0, len(fwdSet))
		for id := range fwdSet {
			fwdIDs = append(fwdIDs, id)
		}
		q, args, err := sqlx.In(`SELECT id, tunnel_id FROM forward WHERE id IN (?)`, fwdIDs)
		if err == nil {
			q = s.DB.Rebind(q)
			var rows []struct {
				ID       int64 `db:"id"`
				TunnelID int64 `db:"tunnel_id"`
			}
			if err := s.DB.Select(&rows, q, args...); err == nil {
				for _, r := range rows {
					tunnelOfForward[r.ID] = r.TunnelID
				}
			}
		}
	}
	tunSet := make(map[int64]struct{})
	for _, tid := range tunnelOfForward {
		tunSet[tid] = struct{}{}
	}
	ratioOfTunnel := make(map[int64]float64)
	if len(tunSet) > 0 {
		tunIDs := make([]int64, 0, len(tunSet))
		for id := range tunSet {
			tunIDs = append(tunIDs, id)
		}
		q, args, err := sqlx.In(`SELECT id, traffic_ratio, flow FROM tunnel WHERE id IN (?)`, tunIDs)
		if err == nil {
			q = s.DB.Rebind(q)
			var rows []struct {
				ID           int64   `db:"id"`
				TrafficRatio float64 `db:"traffic_ratio"`
				Flow         int64   `db:"flow"`
			}
			if err := s.DB.Select(&rows, q, args...); err == nil {
				for _, r := range rows {
					ratio := r.TrafficRatio
					if ratio <= 0 {
						ratio = 1
					}
					tf := float64(r.Flow)
					if tf <= 0 {
						tf = 1
					}
					ratioOfTunnel[r.ID] = ratio * tf
				}
			}
		}
	}
	type du struct{ d, u int64 }
	fwdAgg := make(map[int64]*du)
	userAgg := make(map[int64]*du)
	utAgg := make(map[int64]*du)
	addAgg := func(m map[int64]*du, id int64, d, u int64) {
		v, ok := m[id]
		if !ok {
			v = &du{}
			m[id] = v
		}
		v.d += d
		v.u += u
	}
	checkUsers := make(map[int64]struct{})
	utUsers := make(map[int64]int64)
	for _, p := range parsedItems {
		mult := 1.0
		if tid, ok := tunnelOfForward[p.forwardID]; ok {
			if m, ok := ratioOfTunnel[tid]; ok {
				mult = m
			}
		}
		d := int64(float64(p.d) * mult)
		u := int64(float64(p.u) * mult)
		addAgg(fwdAgg, p.forwardID, d, u)
		addAgg(userAgg, p.userID, d, u)
		// 超限检测（管理员 userTunnelId=0 跳过，与逐条路径一致）
		if p.utID != 0 {
			addAgg(utAgg, p.utID, d, u)
			checkUsers[p.userID] = struct{}{}
			if _, ok := utUsers[p.utID]; !ok {
				utUsers[p.utID] = p.userID
			}
		}
	}
	for id, v := range fwdAgg {
		s.updateForwardFlow(id, v.d, v.u)
	}
	for id, v := range userAgg {
		s.updateUserFlow(id, v.d, v.u)
	}
	for id, v := range utAgg {
		s.updateUserTunnelFlow(id, v.d, v.u)
	}
	for uid := range checkUsers {
		s.checkUserRelatedLimits(uid)
	}
	for utID, uid := range utUsers {
		s.checkUserTunnelRelatedLimits(utID, uid)
	}
}

// parseFlowName 解析 "{forwardId}_{userId}_{userTunnelId}[_tcp|_udp]" 服务名
func parseFlowName(n string) (forwardID, userID, userTunnelID int64, ok bool) {
	forwardID, userID, userTunnelID, ok = gost.ParseForwardServiceName(n)
	if ok {
		return forwardID, userID, userTunnelID, true
	}
	// 兼容 Java split("_") 前三段
	parts := strings.Split(n, "_")
	if len(parts) < 3 {
		return 0, 0, 0, false
	}
	var err error
	forwardID, err = strconv.ParseInt(parts[0], 10, 64)
	if err != nil {
		return 0, 0, 0, false
	}
	userID, err = strconv.ParseInt(parts[1], 10, 64)
	if err != nil {
		return 0, 0, 0, false
	}
	userTunnelID, err = strconv.ParseInt(parts[2], 10, 64)
	if err != nil {
		return 0, 0, 0, false
	}
	return forwardID, userID, userTunnelID, true
}

func (s *FlowService) processFlowData(flow FlowDto) {
	forwardID, userID, userTunnelID, ok := parseFlowName(flow.N)
	if !ok {
		return
	}

	// 倍率：d/u *= trafficRatio * tunnel.flow
	var forward model.Forward
	if err := s.DB.Get(&forward, `SELECT * FROM forward WHERE id = ?`, forwardID); err == nil {
		var tunnel model.Tunnel
		if err := s.DB.Get(&tunnel, `SELECT * FROM tunnel WHERE id = ?`, forward.TunnelID); err == nil {
			ratio := tunnel.TrafficRatio
			if ratio <= 0 {
				ratio = 1
			}
			tf := float64(tunnel.Flow)
			if tf <= 0 {
				tf = 1
			}
			flow.D = int64(float64(flow.D) * ratio * tf)
			flow.U = int64(float64(flow.U) * ratio * tf)
		}
	}

	s.updateForwardFlow(forwardID, flow.D, flow.U)
	s.updateUserFlow(userID, flow.D, flow.U)
	if userTunnelID != 0 {
		s.updateUserTunnelFlow(userTunnelID, flow.D, flow.U)
	}

	// 超限检测（管理员 userTunnelId=0 跳过）
	if userTunnelID != 0 {
		s.checkUserRelatedLimits(userID)
		s.checkUserTunnelRelatedLimits(userTunnelID, userID)
	}
}

func (s *FlowService) lockFor(m *sync.Map, id int64) *sync.Mutex {
	v, _ := m.LoadOrStore(id, &sync.Mutex{})
	return v.(*sync.Mutex)
}

func (s *FlowService) updateForwardFlow(forwardID, d, u int64) {
	lk := s.lockFor(&s.forwardLocks, forwardID)
	lk.Lock()
	defer lk.Unlock()
	_, _ = s.DB.Exec(`UPDATE forward SET in_flow = in_flow + ?, out_flow = out_flow + ? WHERE id = ?`, d, u, forwardID)
}

func (s *FlowService) updateUserFlow(userID, d, u int64) {
	lk := s.lockFor(&s.userLocks, userID)
	lk.Lock()
	defer lk.Unlock()
	_, _ = s.DB.Exec(`UPDATE user SET in_flow = in_flow + ?, out_flow = out_flow + ? WHERE id = ?`, d, u, userID)
}

func (s *FlowService) updateUserTunnelFlow(userTunnelID, d, u int64) {
	lk := s.lockFor(&s.tunnelLocks, userTunnelID)
	lk.Lock()
	defer lk.Unlock()
	_, _ = s.DB.Exec(`UPDATE user_tunnel SET in_flow = in_flow + ?, out_flow = out_flow + ? WHERE id = ?`, d, u, userTunnelID)
}

func (s *FlowService) checkUserRelatedLimits(userID int64) {
	var user model.User
	if err := s.DB.Get(&user, `SELECT * FROM user WHERE id = ?`, userID); err != nil {
		return
	}
	now := time.Now().UnixMilli()
	// 无限流量：跳过配额检查，仅检查到期/停用
	if !model.IsUnlimitedFlow(user.Flow) {
		limit := user.Flow * bytesToGB
		current := user.InFlow + user.OutFlow
		if limit < current {
			slog.Info("流量上报自动暂停", "scope", "user", "user", userID,
				"quotaGB", user.Flow, "used", current, "expTime", user.ExpTime, "status", user.Status)
			s.pauseAllUserServices(userID)
			return
		}
	}
	if (user.ExpTime > 0 && user.ExpTime <= now) || user.Status != 1 {
		slog.Info("流量上报自动暂停", "scope", "user", "user", userID,
			"quotaGB", user.Flow, "used", user.InFlow+user.OutFlow, "expTime", user.ExpTime, "status", user.Status)
		s.pauseAllUserServices(userID)
	}
}

func (s *FlowService) checkUserTunnelRelatedLimits(userTunnelID, userID int64) {
	var ut model.UserTunnel
	if err := s.DB.Get(&ut, `SELECT * FROM user_tunnel WHERE id = ?`, userTunnelID); err != nil {
		return
	}
	flow := ut.InFlow + ut.OutFlow
	now := time.Now().UnixMilli()
	// 无限流量：跳过配额检查，仅检查到期/停用
	if !model.IsUnlimitedFlow(ut.Flow) {
		if flow >= ut.Flow*bytesToGB {
			slog.Info("流量上报自动暂停", "scope", "user_tunnel", "ut", userTunnelID, "user", userID,
				"quotaGB", ut.Flow, "used", flow, "expTime", ut.ExpTime, "status", ut.Status)
			s.pauseSpecificForward(int64(ut.TunnelID), userID)
			return
		}
	}
	if (ut.ExpTime > 0 && ut.ExpTime <= now) || ut.Status != 1 {
		slog.Info("流量上报自动暂停", "scope", "user_tunnel", "ut", userTunnelID, "user", userID,
			"quotaGB", ut.Flow, "used", flow, "expTime", ut.ExpTime, "status", ut.Status)
		s.pauseSpecificForward(int64(ut.TunnelID), userID)
	}
}

func (s *FlowService) pauseAllUserServices(userID int64) {
	var forwards []model.Forward
	// 只处理运行中的，已暂停的不再重复下发
	if err := s.DB.Select(&forwards, `SELECT * FROM forward WHERE user_id = ? AND status = 1`, userID); err != nil {
		return
	}
	if len(forwards) > 0 {
		slog.Info("执行自动暂停", "scope", "user", "user", userID, "count", len(forwards))
	}
	s.pauseForwards(forwards)
}

func (s *FlowService) pauseSpecificForward(tunnelID, userID int64) {
	var forwards []model.Forward
	if err := s.DB.Select(&forwards, `SELECT * FROM forward WHERE tunnel_id = ? AND user_id = ? AND status = 1`, tunnelID, userID); err != nil {
		return
	}
	if len(forwards) > 0 {
		slog.Info("执行自动暂停", "scope", "user_tunnel", "tunnel", tunnelID, "user", userID, "count", len(forwards))
	}
	s.pauseForwards(forwards)
}

// pauseForwards 对每条 forward 用正确 serviceName 暂停：归属权限与入口链
// 一次查回，WS 限并发并行，状态一次批量写。
// 原逐条路径每条转发 1 次权限查询 + 1 次链查询 + E 次 WS + 1 次状态写。
func (s *FlowService) pauseForwards(forwards []model.Forward) {
	if len(forwards) == 0 {
		return
	}
	tunSet := make(map[int64]struct{}, len(forwards))
	userSet := make(map[int]struct{}, len(forwards))
	for i := range forwards {
		tunSet[int64(forwards[i].TunnelID)] = struct{}{}
		userSet[forwards[i].UserID] = struct{}{}
	}
	tunIDs := make([]int64, 0, len(tunSet))
	for id := range tunSet {
		tunIDs = append(tunIDs, id)
	}
	userIDs := make([]int, 0, len(userSet))
	for id := range userSet {
		userIDs = append(userIDs, id)
	}
	// 归属权限一次查回（超集 + 内存精确匹配）
	utByPair := make(map[string]*model.UserTunnel)
	{
		q, args, err := sqlx.In(`SELECT * FROM user_tunnel WHERE tunnel_id IN (?) AND user_id IN (?)`, tunIDs, userIDs)
		if err != nil {
			s.pauseForwardsLegacy(forwards)
			return
		}
		q = s.DB.Rebind(q)
		var uts []model.UserTunnel
		if err := s.DB.Select(&uts, q, args...); err != nil {
			s.pauseForwardsLegacy(forwards)
			return
		}
		for i := range uts {
			ut := uts[i]
			if _, ok := tunSet[int64(ut.TunnelID)]; !ok {
				continue
			}
			if _, ok := userSet[ut.UserID]; !ok {
				continue
			}
			cp := ut
			utByPair[utKey(ut.UserID, ut.TunnelID)] = &cp
		}
	}
	// 入口链一次查回
	chainsByTunnel := make(map[int64][]model.ChainTunnel)
	{
		q, args, err := sqlx.In(`SELECT * FROM chain_tunnel WHERE tunnel_id IN (?) AND (chain_type = '1' OR chain_type = 1)`, tunIDs)
		if err != nil {
			s.pauseForwardsLegacy(forwards)
			return
		}
		q = s.DB.Rebind(q)
		var chains []model.ChainTunnel
		if err := s.DB.Select(&chains, q, args...); err != nil {
			s.pauseForwardsLegacy(forwards)
			return
		}
		for _, ct := range chains {
			chainsByTunnel[ct.TunnelID] = append(chainsByTunnel[ct.TunnelID], ct)
		}
	}
	type pauseTask struct {
		fwID     int64
		baseName string
		nodeIDs  []int64
	}
	tasks := make([]pauseTask, 0, len(forwards))
	ids := make([]int64, 0, len(forwards))
	for i := range forwards {
		fw := &forwards[i]
		var utID int64
		if ut := utByPair[utKey(fw.UserID, fw.TunnelID)]; ut != nil {
			utID = int64(ut.ID)
		}
		base := gost.ForwardServiceBase(fw.ID, int64(fw.UserID), utID)
		var nodeIDs []int64
		for _, ct := range chainsByTunnel[int64(fw.TunnelID)] {
			nodeIDs = append(nodeIDs, ct.NodeID)
		}
		tasks = append(tasks, pauseTask{fwID: fw.ID, baseName: base, nodeIDs: nodeIDs})
		ids = append(ids, fw.ID)
	}
	_ = parallelEach(len(tasks), batchConcurrency, func(i int) string {
		t := tasks[i]
		payload := gost.PauseResumePayload(t.baseName)
		for _, nodeID := range t.nodeIDs {
			if s.Hub == nil {
				continue
			}
			res := s.Hub.SendMsg(nodeID, payload, "PauseService")
			if !gost.IsOK(res.Msg) {
				slog.Info("PauseService", "node", nodeID, "name", t.baseName, "msg", res.Msg)
			}
		}
		return ""
	})
	// 状态一次批量写（分片单语句 IN；原逐条 UPDATE）
	for start := 0; start < len(ids); start += 500 {
		end := start + 500
		if end > len(ids) {
			end = len(ids)
		}
		q, args, err := sqlx.In(`UPDATE forward SET status = 0 WHERE id IN (?)`, ids[start:end])
		if err != nil {
			continue
		}
		q = s.DB.Rebind(q)
		_, _ = s.DB.Exec(q, args...)
	}
}

// pauseForwardsLegacy 逐条暂停（批量预取失败时的回落路径，与原语义一致）
func (s *FlowService) pauseForwardsLegacy(forwards []model.Forward) {
	for i := range forwards {
		fw := &forwards[i]
		serviceName := s.serviceNameForForward(fw)
		if serviceName == "" {
			// 仍更新状态
			_, _ = s.DB.Exec(`UPDATE forward SET status = 0 WHERE id = ?`, fw.ID)
			continue
		}
		s.pauseServiceOnNodes(int64(fw.TunnelID), serviceName)
		_, _ = s.DB.Exec(`UPDATE forward SET status = 0 WHERE id = ?`, fw.ID)
	}
}

func (s *FlowService) serviceNameForForward(fw *model.Forward) string {
	var ut model.UserTunnel
	err := s.DB.Get(&ut, `SELECT * FROM user_tunnel WHERE user_id = ? AND tunnel_id = ? LIMIT 1`, fw.UserID, fw.TunnelID)
	if err != nil {
		// 管理员可能无 user_tunnel，用 0
		return gost.ForwardServiceBase(fw.ID, int64(fw.UserID), 0)
	}
	return gost.ForwardServiceBase(fw.ID, int64(fw.UserID), int64(ut.ID))
}

func (s *FlowService) pauseServiceOnNodes(tunnelID int64, baseName string) {
	var chains []model.ChainTunnel
	// chain_type 可能是 "1" 或 1
	if err := s.DB.Select(&chains, `SELECT * FROM chain_tunnel WHERE tunnel_id = ? AND (chain_type = '1' OR chain_type = 1)`, tunnelID); err != nil {
		return
	}
	payload := gost.PauseResumePayload(baseName)
	for _, ct := range chains {
		if s.Hub == nil {
			continue
		}
		res := s.Hub.SendMsg(ct.NodeID, payload, "PauseService")
		if !gost.IsOK(res.Msg) {
			slog.Info("PauseService", "node", ct.NodeID, "name", baseName, "msg", res.Msg)
		}
	}
}

// CleanNodeConfigs 异步清理节点孤立配置
func (s *FlowService) CleanNodeConfigs(nodeID int64, cfg GostConfigDto) {
	var node model.Node
	if err := s.DB.Get(&node, `SELECT * FROM node WHERE id = ?`, nodeID); err != nil {
		return
	}
	s.cleanOrphanedServices(cfg.Services, &node)
	s.cleanOrphanedChains(cfg.Chains, &node)
	s.cleanOrphanedLimiters(cfg.Limiters, &node)
}

func (s *FlowService) cleanOrphanedServices(items []ConfigItem, node *model.Node) {
	for _, svc := range items {
		name := svc.Name
		if name == "" || name == "web_api" {
			continue
		}
		parts := strings.Split(name, "_")
		if len(parts) < 2 {
			continue
		}
		last := parts[len(parts)-1]
		switch last {
		case "tls":
			// {tunnelId}_tls
			tunnelID, err := strconv.ParseInt(parts[0], 10, 64)
			if err != nil {
				continue
			}
			var cnt int
			_ = s.DB.Get(&cnt, `SELECT COUNT(1) FROM tunnel WHERE id = ?`, tunnelID)
			if cnt == 0 {
				s.deleteServices(node.ID, []string{gost.TunnelTLS(tunnelID)})
				slog.Info("删除孤立的服务", "name", name, "node", node.ID)
			}
		case "tcp":
			// {forwardId}_{userId}_{userTunnelId}_tcp
			if len(parts) < 4 {
				continue
			}
			forwardID, err := strconv.ParseInt(parts[0], 10, 64)
			if err != nil {
				continue
			}
			var cnt int
			_ = s.DB.Get(&cnt, `SELECT COUNT(1) FROM forward WHERE id = ?`, forwardID)
			if cnt == 0 {
				base := strings.TrimSuffix(name, "_tcp")
				s.deleteServices(node.ID, []string{base + "_tcp", base + "_udp"})
				slog.Info("删除孤立的服务", "name", name, "node", node.ID)
			}
		}
	}
}

func (s *FlowService) cleanOrphanedChains(items []ConfigItem, node *model.Node) {
	for _, ch := range items {
		parts := strings.Split(ch.Name, "_")
		if len(parts) == 0 {
			continue
		}
		tunnelID, err := strconv.ParseInt(parts[len(parts)-1], 10, 64)
		if err != nil {
			continue
		}
		var cnt int
		_ = s.DB.Get(&cnt, `SELECT COUNT(1) FROM tunnel WHERE id = ?`, tunnelID)
		if cnt == 0 {
			s.deleteChains(node.ID, ch.Name)
			slog.Info("删除孤立的链", "name", ch.Name, "node", node.ID)
		}
	}
}

func (s *FlowService) cleanOrphanedLimiters(items []ConfigItem, node *model.Node) {
	for _, lim := range items {
		id, err := strconv.ParseInt(lim.Name, 10, 64)
		if err != nil {
			continue
		}
		var cnt int
		_ = s.DB.Get(&cnt, `SELECT COUNT(1) FROM speed_limit WHERE id = ?`, id)
		if cnt == 0 {
			s.deleteLimiters(node.ID, id)
			slog.Info("删除孤立的限流器", "name", lim.Name, "node", node.ID)
		}
	}
}

func (s *FlowService) deleteServices(nodeID int64, names []string) {
	if s.Hub == nil || len(names) == 0 {
		return
	}
	// 孤儿清理无需等待节点确认
	s.Hub.SendNoWait(nodeID, gost.DeleteServicePayload(names), "DeleteService")
}

func (s *FlowService) deleteChains(nodeID int64, name string) {
	if s.Hub == nil {
		return
	}
	s.Hub.SendNoWait(nodeID, map[string]any{"chain": name}, "DeleteChains")
}

func (s *FlowService) deleteLimiters(nodeID, name int64) {
	if s.Hub == nil {
		return
	}
	s.Hub.SendNoWait(nodeID, map[string]any{"limiter": strconv.FormatInt(name, 10)}, "DeleteLimiters")
}

// PauseForwardByIDs 供定时任务调用：按用户/隧道到期暂停，使用正确 serviceName
func (s *FlowService) PauseForwardByIDs(forwards []model.Forward) {
	s.pauseForwards(forwards)
}
