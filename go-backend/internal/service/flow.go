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

// Upload 处理流量上报数组
func (s *FlowService) Upload(items []FlowDto) {
	for _, item := range items {
		if item.N == "" || item.N == "web_api" {
			continue
		}
		s.processFlowData(item)
	}
}

func (s *FlowService) processFlowData(flow FlowDto) {
	forwardID, userID, userTunnelID, ok := gost.ParseForwardServiceName(flow.N)
	if !ok {
		// 兼容 Java split("_") 前三段
		parts := strings.Split(flow.N, "_")
		if len(parts) < 3 {
			return
		}
		var err error
		forwardID, err = strconv.ParseInt(parts[0], 10, 64)
		if err != nil {
			return
		}
		userID, err = strconv.ParseInt(parts[1], 10, 64)
		if err != nil {
			return
		}
		userTunnelID, err = strconv.ParseInt(parts[2], 10, 64)
		if err != nil {
			return
		}
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
	limit := user.Flow * bytesToGB
	current := user.InFlow + user.OutFlow
	now := time.Now().UnixMilli()
	if limit < current || (user.ExpTime > 0 && user.ExpTime <= now) || user.Status != 1 {
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
	if flow >= ut.Flow*bytesToGB || (ut.ExpTime > 0 && ut.ExpTime <= now) || ut.Status != 1 {
		s.pauseSpecificForward(int64(ut.TunnelID), userID)
	}
}

func (s *FlowService) pauseAllUserServices(userID int64) {
	var forwards []model.Forward
	if err := s.DB.Select(&forwards, `SELECT * FROM forward WHERE user_id = ?`, userID); err != nil {
		return
	}
	s.pauseForwards(forwards)
}

func (s *FlowService) pauseSpecificForward(tunnelID, userID int64) {
	var forwards []model.Forward
	if err := s.DB.Select(&forwards, `SELECT * FROM forward WHERE tunnel_id = ? AND user_id = ?`, tunnelID, userID); err != nil {
		return
	}
	s.pauseForwards(forwards)
}

// pauseForwards 对每条 forward 用正确 serviceName 暂停
func (s *FlowService) pauseForwards(forwards []model.Forward) {
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
	res := s.Hub.SendMsg(nodeID, gost.DeleteServicePayload(names), "DeleteService")
	_ = gost.NormalizeOK(res.Msg)
}

func (s *FlowService) deleteChains(nodeID int64, name string) {
	if s.Hub == nil {
		return
	}
	res := s.Hub.SendMsg(nodeID, map[string]any{"chain": name}, "DeleteChains")
	_ = gost.NormalizeOK(res.Msg)
}

func (s *FlowService) deleteLimiters(nodeID, name int64) {
	if s.Hub == nil {
		return
	}
	res := s.Hub.SendMsg(nodeID, map[string]any{"limiter": strconv.FormatInt(name, 10)}, "DeleteLimiters")
	_ = gost.NormalizeOK(res.Msg)
}

// PauseForwardByIDs 供定时任务调用：按用户/隧道到期暂停，使用正确 serviceName
func (s *FlowService) PauseForwardByIDs(forwards []model.Forward) {
	s.pauseForwards(forwards)
}
