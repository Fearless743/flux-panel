package service

import (
	"encoding/json"
	"fmt"
	"sort"
	"strings"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/gost"
	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/repo"
	"github.com/Fearless743/flux-panel/go-backend/internal/ws"
	"github.com/jmoiron/sqlx"
)

// TunnelService 对齐 Java TunnelServiceImpl
type TunnelService struct {
	DB       *sqlx.DB
	Hub      *ws.Hub
	Tunnel   *repo.TunnelRepo
	Chain    *repo.ChainTunnelRepo
	Node     *repo.NodeRepo
	Forward  *repo.ForwardRepo
	Port     *repo.ForwardPortRepo
	UserTunn *repo.UserTunnelRepo
}

func NewTunnelService(db *sqlx.DB, hub *ws.Hub) *TunnelService {
	return &TunnelService{
		DB:       db,
		Hub:      hub,
		Tunnel:   repo.NewTunnelRepo(db),
		Chain:    repo.NewChainTunnelRepo(db),
		Node:     repo.NewNodeRepo(db),
		Forward:  repo.NewForwardRepo(db),
		Port:     repo.NewForwardPortRepo(db),
		UserTunn: repo.NewUserTunnelRepo(db),
	}
}

// ---- DTO ----

// ChainTunnelInput 前端传入的拓扑节点（不信任 id/port）
type ChainTunnelInput struct {
	NodeID      int64   `json:"nodeId"`
	Protocol    *string `json:"protocol"`
	Strategy    *string `json:"strategy"`
	ExitNodeIDs []int64 `json:"exitNodeIds"` // 入口节点使用的出口节点 ID 列表
	// 兼容可能带的字段
	ChainType any `json:"chainType"`
	Port      any `json:"port"`
	Inx       any `json:"inx"`
	ID        any `json:"id"`
}

type TunnelCreateReq struct {
	Name         string               `json:"name"`
	Type         int                  `json:"type"`
	Flow         int                  `json:"flow"`
	TrafficRatio float64              `json:"trafficRatio"`
	InIP         string               `json:"inIp"`
	InNodeID     []ChainTunnelInput   `json:"inNodeId"`
	ChainNodes   [][]ChainTunnelInput `json:"chainNodes"`
	OutNodeID    []ChainTunnelInput   `json:"outNodeId"`
}

type TunnelUpdateReq struct {
	ID           int64                `json:"id"`
	Name         string               `json:"name"`
	Flow         int                  `json:"flow"`
	TrafficRatio float64              `json:"trafficRatio"`
	InIP         string               `json:"inIp"`
	InNodeID     []ChainTunnelInput   `json:"inNodeId"`
	ChainNodes   [][]ChainTunnelInput `json:"chainNodes"`
	OutNodeID    []ChainTunnelInput   `json:"outNodeId"`
}

type TunnelDetailDto struct {
	ID           int64               `json:"id"`
	Name         string              `json:"name"`
	Type         int                 `json:"type"`
	Flow         int                 `json:"flow"`
	TrafficRatio float64             `json:"trafficRatio"`
	Status       int                 `json:"status"`
	CreatedTime  int64               `json:"createdTime"`
	UpdatedTime  int64               `json:"updatedTime"`
	InIP         *string             `json:"inIp"`
	InNodeID     []model.ChainTunnel `json:"inNodeId"`
	ChainNodes   [][]model.ChainTunnel `json:"chainNodes"`
	OutNodeID    []model.ChainTunnel `json:"outNodeId"`
}

// ---- create ----

func (s *TunnelService) Create(req TunnelCreateReq) error {
	if strings.TrimSpace(req.Name) == "" {
		return fmt.Errorf("隧道名称不能为空")
	}
	if len(req.InNodeID) == 0 {
		return fmt.Errorf("入口节点不能为空")
	}
	if req.Type != 1 && req.Type != 2 {
		return fmt.Errorf("隧道类型不合法")
	}
	if req.Type == 2 && len(req.OutNodeID) == 0 {
		return fmt.Errorf("出口不能为空")
	}
	if req.TrafficRatio <= 0 || req.TrafficRatio > 100 {
		return fmt.Errorf("流量倍率必须大于0.0且不超过100.0")
	}

	cnt, err := s.Tunnel.CountByName(req.Name)
	if err != nil {
		return err
	}
	if cnt > 0 {
		return fmt.Errorf("隧道名称重复")
	}

	nodes := make(map[int64]*model.Node)
	var chainTunnels []model.ChainTunnel
	var nodeIDs []int64

	// 入口
	for _, in := range req.InNodeID {
		if in.NodeID == 0 {
			return fmt.Errorf("节点不存在")
		}
		nodeIDs = append(nodeIDs, in.NodeID)
		n, err := s.Node.GetByID(in.NodeID)
		if err != nil {
			return err
		}
		if n == nil {
			return fmt.Errorf("节点不存在")
		}
		nodes[n.ID] = n
		// 将 ExitNodeIDs 转换为 JSON 字符串
		var exitNodeIDs *string
		if len(in.ExitNodeIDs) > 0 {
			jsonBytes, err := json.Marshal(in.ExitNodeIDs)
			if err != nil {
				return fmt.Errorf("序列化出口节点 ID 失败: %w", err)
			}
			s := string(jsonBytes)
			exitNodeIDs = &s
		}
		ct := repo.NewChainTunnelWithExitBinding(0, 1, in.NodeID, nil, nil, nil, nil, exitNodeIDs)
		chainTunnels = append(chainTunnels, ct)
	}

	// type2: 转发链 + 出口
	var chainGroups [][]model.ChainTunnel
	var outNodes []model.ChainTunnel
	if req.Type == 2 {
		inx := 1
		for _, group := range req.ChainNodes {
			var newGroup []model.ChainTunnel
			for _, c := range group {
				if c.NodeID == 0 {
					return fmt.Errorf("节点不存在")
				}
				nodeIDs = append(nodeIDs, c.NodeID)
				n, err := s.Node.GetByID(c.NodeID)
				if err != nil {
					return err
				}
				if n == nil {
					return fmt.Errorf("节点不存在")
				}
				nodes[n.ID] = n
				port, err := s.GetNodePort(c.NodeID)
				if err != nil {
					return err
				}
				p := port
				ct := repo.NewChainTunnel(0, 2, c.NodeID, &p, c.Strategy, c.Protocol, repo.IntPtr(inx))
				newGroup = append(newGroup, ct)
				chainTunnels = append(chainTunnels, ct)
			}
			if len(newGroup) > 0 {
				chainGroups = append(chainGroups, newGroup)
				inx++
			}
		}
		for _, out := range req.OutNodeID {
			if out.NodeID == 0 {
				return fmt.Errorf("节点不存在")
			}
			nodeIDs = append(nodeIDs, out.NodeID)
			n, err := s.Node.GetByID(out.NodeID)
			if err != nil {
				return err
			}
			if n == nil {
				return fmt.Errorf("节点不存在")
			}
			nodes[n.ID] = n
			port, err := s.GetNodePort(out.NodeID)
			if err != nil {
				return err
			}
			p := port
			ct := repo.NewChainTunnel(0, 3, out.NodeID, &p, out.Strategy, out.Protocol, nil)
			outNodes = append(outNodes, ct)
			chainTunnels = append(chainTunnels, ct)
		}
	}

	// 节点不重复
	set := make(map[int64]struct{})
	for _, id := range nodeIDs {
		if _, ok := set[id]; ok {
			return fmt.Errorf("节点重复")
		}
		set[id] = struct{}{}
	}
	// 全部在线
	for id := range set {
		n := nodes[id]
		if n == nil {
			return fmt.Errorf("部分节点不存在")
		}
		if n.Status != 1 {
			return fmt.Errorf("部分节点不在线")
		}
	}

	// in_ip
	inIP := strings.TrimSpace(req.InIP)
	if inIP == "" {
		var parts []string
		for _, in := range req.InNodeID {
			n := nodes[in.NodeID]
			if n != nil && n.ServerIP != "" {
				parts = append(parts, n.ServerIP)
			}
		}
		inIP = strings.Join(parts, ",")
	}

	now := time.Now().UnixMilli()
	tunnel := &model.Tunnel{
		Name:         req.Name,
		TrafficRatio: req.TrafficRatio,
		Type:         req.Type,
		Protocol:     "tls",
		Flow:         req.Flow,
		CreatedTime:  now,
		UpdatedTime:  now,
		Status:       1,
		InIP:         &inIP,
	}
	if err := s.Tunnel.Insert(tunnel); err != nil {
		return err
	}
	for i := range chainTunnels {
		chainTunnels[i].TunnelID = tunnel.ID
	}
	// 同步 chainGroups / outNodes tunnelID
	for gi := range chainGroups {
		for i := range chainGroups[gi] {
			chainGroups[gi][i].TunnelID = tunnel.ID
		}
	}
	for i := range outNodes {
		outNodes[i].TunnelID = tunnel.ID
	}
	// 入口列表（用于下发）
	entries := make([]model.ChainTunnel, 0, len(req.InNodeID))
	for i := range chainTunnels {
		if repo.ChainTypeOf(chainTunnels[i]) == 1 {
			entries = append(entries, chainTunnels[i])
		}
	}

	if err := s.Chain.InsertBatch(chainTunnels); err != nil {
		_ = s.Tunnel.Delete(tunnel.ID)
		return err
	}

	// type2 下发
	if tunnel.Type == 2 {
		type successRef struct {
			nodeID int64
			kind   string // chain / service
			name   string
		}
		var success []successRef
		rollback := func() {
			_ = s.Tunnel.Delete(tunnel.ID)
			_ = s.Chain.DeleteByTunnelID(tunnel.ID)
			for _, r := range success {
				if r.kind == "chain" {
					deleteChains(s.Hub, r.nodeID, r.name)
				} else {
					deleteService(s.Hub, r.nodeID, []string{r.name})
				}
			}
		}

		// 入口 → 第一跳 / 出口
		for _, entry := range entries {
			var target []model.ChainTunnel
			if len(chainGroups) == 0 {
				// 无转发链时，根据入口的 ExitNodeIDs 过滤出口节点
				target = filterExitsByEntry(outNodes, entry)
			} else {
				target = chainGroups[0]
			}
			res := s.addChains(entry.NodeID, target, nodes)
			if !gost.IsOK(res.Msg) {
				// 无转发链时 Java isError 为空，这里仍按失败回滚更安全
				if len(chainGroups) == 0 {
					// 对齐 Java isError 空实现：忽略
					continue
				}
				rollback()
				return fmt.Errorf("%s", gost.NormalizeOK(res.Msg))
			}
			success = append(success, successRef{nodeID: entry.NodeID, kind: "chain", name: gost.ChainName(tunnel.ID)})
		}

		// 转发链
		for i := range chainGroups {
			var target []model.ChainTunnel
			if i+1 < len(chainGroups) {
				target = chainGroups[i+1]
			} else {
				target = outNodes
			}
			for _, ct := range chainGroups[i] {
				res := s.addChains(ct.NodeID, target, nodes)
				if !gost.IsOK(res.Msg) {
					rollback()
					return fmt.Errorf("%s", gost.NormalizeOK(res.Msg))
				}
				success = append(success, successRef{nodeID: ct.NodeID, kind: "chain", name: gost.ChainName(tunnel.ID)})

				res = s.addChainService(ct.NodeID, ct, nodes)
				if !gost.IsOK(res.Msg) {
					rollback()
					return fmt.Errorf("%s", gost.NormalizeOK(res.Msg))
				}
				success = append(success, successRef{nodeID: ct.NodeID, kind: "service", name: gost.TunnelTLS(tunnel.ID)})
			}
		}

		// 出口
		for _, ct := range outNodes {
			res := s.addChainService(ct.NodeID, ct, nodes)
			if !gost.IsOK(res.Msg) {
				rollback()
				return fmt.Errorf("%s", gost.NormalizeOK(res.Msg))
			}
			success = append(success, successRef{nodeID: ct.NodeID, kind: "service", name: gost.TunnelTLS(tunnel.ID)})
		}
	}
	return nil
}

// ---- list ----

func (s *TunnelService) List() ([]TunnelDetailDto, error) {
	tunnels, err := s.Tunnel.ListAll()
	if err != nil {
		return nil, err
	}
	if len(tunnels) == 0 {
		return []TunnelDetailDto{}, nil
	}
	ids := make([]int64, 0, len(tunnels))
	for _, t := range tunnels {
		ids = append(ids, t.ID)
	}
	allCT, err := s.Chain.ListByTunnelIDs(ids)
	if err != nil {
		return nil, err
	}
	byTunnel := make(map[int64][]model.ChainTunnel)
	for _, ct := range allCT {
		byTunnel[ct.TunnelID] = append(byTunnel[ct.TunnelID], ct)
	}

	out := make([]TunnelDetailDto, 0, len(tunnels))
	for _, t := range tunnels {
		dto := TunnelDetailDto{
			ID:           t.ID,
			Name:         t.Name,
			Type:         t.Type,
			Flow:         t.Flow,
			TrafficRatio: t.TrafficRatio,
			Status:       t.Status,
			CreatedTime:  t.CreatedTime,
			UpdatedTime:  t.UpdatedTime,
			InIP:         t.InIP,
			InNodeID:     []model.ChainTunnel{},
			ChainNodes:   [][]model.ChainTunnel{},
			OutNodeID:    []model.ChainTunnel{},
		}
		cts := byTunnel[t.ID]
		entries, chains, outs := splitTopology(cts)
		dto.InNodeID = entries
		dto.ChainNodes = chains
		dto.OutNodeID = outs
		out = append(out, dto)
	}
	return out, nil
}

// ---- update ----

func (s *TunnelService) Update(req TunnelUpdateReq) error {
	if req.ID == 0 {
		return fmt.Errorf("隧道ID不能为空")
	}
	if strings.TrimSpace(req.Name) == "" {
		return fmt.Errorf("隧道名称不能为空")
	}
	existing, err := s.Tunnel.GetByID(req.ID)
	if err != nil {
		return err
	}
	if existing == nil {
		return fmt.Errorf("隧道不存在")
	}

	if len(req.InNodeID) > 0 {
		if err := s.reconfigureTunnelNodes(existing, req); err != nil {
			return err
		}
	}

	inIP := strings.TrimSpace(req.InIP)
	if inIP == "" {
		entries, err := s.Chain.ListByTunnelID(req.ID)
		if err != nil {
			return err
		}
		var parts []string
		for _, ct := range entries {
			if repo.ChainTypeOf(ct) != 1 {
				continue
			}
			n, err := s.Node.GetByID(ct.NodeID)
			if err != nil {
				return err
			}
			if n == nil {
				return fmt.Errorf("隧道节点数据错误，部分节点不存在")
			}
			parts = append(parts, n.ServerIP)
		}
		inIP = strings.Join(parts, ",")
	}
	now := time.Now().UnixMilli()
	return s.Tunnel.UpdateBasic(req.ID, req.Name, req.Flow, req.TrafficRatio, &inIP, now)
}

func (s *TunnelService) reconfigureTunnelNodes(tunnel *model.Tunnel, dto TunnelUpdateReq) error {
	tunnelID := tunnel.ID
	isTunnelForward := tunnel.Type == 2

	chainGroupsReq := dto.ChainNodes
	outNodesReq := dto.OutNodeID
	if isTunnelForward {
		if len(outNodesReq) == 0 {
			return fmt.Errorf("出口不能为空")
		}
	}

	// 规范化
	var newEntries []model.ChainTunnel
	var newChains [][]model.ChainTunnel
	var newOuts []model.ChainTunnel
	var nodeIDs []int64

	for _, in := range dto.InNodeID {
		if in.NodeID == 0 {
			return fmt.Errorf("节点不存在")
		}
		// 将 ExitNodeIDs 转换为 JSON 字符串
		var exitNodeIDs *string
		if len(in.ExitNodeIDs) > 0 {
			jsonBytes, err := json.Marshal(in.ExitNodeIDs)
			if err != nil {
				return fmt.Errorf("序列化出口节点 ID 失败: %w", err)
			}
			s := string(jsonBytes)
			exitNodeIDs = &s
		}
		newEntries = append(newEntries, repo.NewChainTunnelWithExitBinding(tunnelID, 1, in.NodeID, nil, nil, nil, nil, exitNodeIDs))
		nodeIDs = append(nodeIDs, in.NodeID)
	}
	inx := 1
	for _, group := range chainGroupsReq {
		var newGroup []model.ChainTunnel
		for _, c := range group {
			if c.NodeID == 0 {
				return fmt.Errorf("节点不存在")
			}
			ct := repo.NewChainTunnel(tunnelID, 2, c.NodeID, nil, c.Strategy, c.Protocol, repo.IntPtr(inx))
			newGroup = append(newGroup, ct)
			nodeIDs = append(nodeIDs, c.NodeID)
		}
		if len(newGroup) > 0 {
			newChains = append(newChains, newGroup)
			inx++
		}
	}
	for _, out := range outNodesReq {
		if out.NodeID == 0 {
			return fmt.Errorf("节点不存在")
		}
		ct := repo.NewChainTunnel(tunnelID, 3, out.NodeID, nil, out.Strategy, out.Protocol, nil)
		newOuts = append(newOuts, ct)
		nodeIDs = append(nodeIDs, out.NodeID)
	}

	set := make(map[int64]struct{})
	for _, id := range nodeIDs {
		if _, ok := set[id]; ok {
			return fmt.Errorf("节点重复")
		}
		set[id] = struct{}{}
	}

	nodes := make(map[int64]*model.Node)
	for id := range set {
		n, err := s.Node.GetByID(id)
		if err != nil {
			return err
		}
		if n == nil {
			return fmt.Errorf("部分节点不存在")
		}
		if n.Status != 1 {
			return fmt.Errorf("节点 %s 不在线，无法下发配置", n.Name)
		}
		nodes[id] = n
	}

	oldRecords, err := s.Chain.ListByTunnelID(tunnelID)
	if err != nil {
		return err
	}
	oldPorts := make(map[int64]int)
	for _, old := range oldRecords {
		if old.Port != nil {
			oldPorts[old.NodeID] = *old.Port
		}
	}

	// 端口沿用
	for gi := range newChains {
		for i := range newChains[gi] {
			ct := &newChains[gi][i]
			if p, ok := oldPorts[ct.NodeID]; ok {
				pp := p
				ct.Port = &pp
			} else {
				port, err := s.GetNodePort(ct.NodeID)
				if err != nil {
					return err
				}
				ct.Port = &port
			}
		}
	}
	for i := range newOuts {
		ct := &newOuts[i]
		if p, ok := oldPorts[ct.NodeID]; ok {
			pp := p
			ct.Port = &pp
		} else {
			port, err := s.GetNodePort(ct.NodeID)
			if err != nil {
				return err
			}
			ct.Port = &port
		}
	}

	var failures []string

	// 清理旧 gost
	if isTunnelForward {
		chainNeeded := make(map[int64]struct{})
		serviceNeeded := make(map[int64]struct{})
		for _, ct := range newEntries {
			chainNeeded[ct.NodeID] = struct{}{}
		}
		for _, group := range newChains {
			for _, ct := range group {
				chainNeeded[ct.NodeID] = struct{}{}
				serviceNeeded[ct.NodeID] = struct{}{}
			}
		}
		for _, ct := range newOuts {
			serviceNeeded[ct.NodeID] = struct{}{}
		}
		chainCleaned := make(map[int64]struct{})
		serviceCleaned := make(map[int64]struct{})
		for _, old := range oldRecords {
			ct := repo.ChainTypeOf(old)
			hadChain := ct == 1 || ct == 2
			hadService := ct == 2 || ct == 3
			if hadChain {
				if _, need := chainNeeded[old.NodeID]; !need {
					if _, done := chainCleaned[old.NodeID]; !done {
						deleteChains(s.Hub, old.NodeID, gost.ChainName(tunnelID))
						chainCleaned[old.NodeID] = struct{}{}
					}
				}
			}
			if hadService {
				if _, need := serviceNeeded[old.NodeID]; !need {
					if _, done := serviceCleaned[old.NodeID]; !done {
						deleteService(s.Hub, old.NodeID, []string{gost.TunnelTLS(tunnelID)})
						serviceCleaned[old.NodeID] = struct{}{}
					}
				}
			}
		}
	}

	// DB 替换
	if err := s.Chain.DeleteByTunnelID(tunnelID); err != nil {
		return err
	}
	allNew := make([]model.ChainTunnel, 0, len(newEntries)+len(newOuts))
	allNew = append(allNew, newEntries...)
	for _, g := range newChains {
		allNew = append(allNew, g...)
	}
	allNew = append(allNew, newOuts...)
	if err := s.Chain.InsertBatch(allNew); err != nil {
		return err
	}

	// 下发
	if isTunnelForward {
		for _, entry := range newEntries {
			var target []model.ChainTunnel
			if len(newChains) == 0 {
				// 无转发链时，根据入口的 ExitNodeIDs 过滤出口节点
				target = filterExitsByEntry(newOuts, entry)
			} else {
				target = newChains[0]
			}
			res := pushChains(s.Hub, entry.NodeID, target, nodes)
			if !gost.IsOK(res.Msg) {
				name := nodeName(nodes, entry.NodeID)
				failures = append(failures, fmt.Sprintf("入口[%s]链下发失败: %s", name, res.Msg))
			}
		}
		for i := range newChains {
			var target []model.ChainTunnel
			if i+1 < len(newChains) {
				target = newChains[i+1]
			} else {
				target = newOuts
			}
			for _, ct := range newChains[i] {
				res := pushChains(s.Hub, ct.NodeID, target, nodes)
				if !gost.IsOK(res.Msg) {
					failures = append(failures, fmt.Sprintf("转发链[%s]链下发失败: %s", nodeName(nodes, ct.NodeID), res.Msg))
				}
				res = pushChainService(s.Hub, ct.NodeID, ct, nodes)
				if !gost.IsOK(res.Msg) {
					failures = append(failures, fmt.Sprintf("转发链[%s]服务下发失败: %s", nodeName(nodes, ct.NodeID), res.Msg))
				}
			}
		}
		for _, ct := range newOuts {
			res := pushChainService(s.Hub, ct.NodeID, ct, nodes)
			if !gost.IsOK(res.Msg) {
				failures = append(failures, fmt.Sprintf("出口[%s]服务下发失败: %s", nodeName(nodes, ct.NodeID), res.Msg))
			}
		}
	}

	// 入口变化 → 迁移转发服务
	oldEntryIDs := make(map[int64]struct{})
	for _, old := range oldRecords {
		if repo.ChainTypeOf(old) == 1 {
			oldEntryIDs[old.NodeID] = struct{}{}
		}
	}
	newEntryIDs := make(map[int64]struct{})
	for _, e := range newEntries {
		newEntryIDs[e.NodeID] = struct{}{}
	}
	s.migrateForwardServices(tunnel, oldEntryIDs, newEntryIDs, nodes, &failures)

	if len(failures) > 0 {
		return fmt.Errorf("配置已保存，但部分下发失败: %s", strings.Join(failures, "；"))
	}
	return nil
}

func (s *TunnelService) migrateForwardServices(
	tunnel *model.Tunnel,
	oldEntryIDs, newEntryIDs map[int64]struct{},
	nodes map[int64]*model.Node,
	failures *[]string,
) {
	removed := make(map[int64]struct{})
	for id := range oldEntryIDs {
		if _, ok := newEntryIDs[id]; !ok {
			removed[id] = struct{}{}
		}
	}
	added := make(map[int64]struct{})
	for id := range newEntryIDs {
		if _, ok := oldEntryIDs[id]; !ok {
			added[id] = struct{}{}
		}
	}
	if len(removed) == 0 && len(added) == 0 {
		return
	}

	forwards, err := s.Forward.ListByTunnelID(tunnel.ID)
	if err != nil {
		*failures = append(*failures, "查询转发失败: "+err.Error())
		return
	}
	fwSvc := NewForwardService(s.DB, s.Hub)

	for i := range forwards {
		fw := &forwards[i]
		ut, _ := s.UserTunn.GetByUserAndTunnel(fw.UserID, int(tunnel.ID))
		utID := int64(0)
		var limiter *int
		if ut != nil {
			utID = int64(ut.ID)
			limiter = ut.SpeedID
		}
		base := gost.ForwardServiceBase(fw.ID, int64(fw.UserID), utID)

		// preferred port
		var preferredPort *int
		existingPorts, _ := s.Port.ListByForwardID(fw.ID)
		for _, fp := range existingPorts {
			p := fp.Port
			if preferredPort == nil {
				preferredPort = &p
			}
			if _, wasRemoved := removed[fp.NodeID]; !wasRemoved {
				preferredPort = &p
				break
			}
		}

		for nodeID := range removed {
			deleteService(s.Hub, nodeID, []string{base + "_tcp", base + "_udp"})
			_ = s.Port.DeleteByForwardAndNode(fw.ID, nodeID)
		}

		for nodeID := range added {
			n := nodes[nodeID]
			if n == nil {
				n2, _ := s.Node.GetByID(nodeID)
				n = n2
			}
			if n == nil {
				*failures = append(*failures, fmt.Sprintf("转发[%s]节点不存在", fw.Name))
				continue
			}
			available, err := s.GetAvailablePorts(nodeID)
			if err != nil {
				*failures = append(*failures, fmt.Sprintf("转发[%s]在节点[%s]分配端口失败: %s", fw.Name, n.Name, err.Error()))
				continue
			}
			if len(available) == 0 {
				*failures = append(*failures, fmt.Sprintf("转发[%s]在节点[%s]分配端口失败: 节点端口已满，无可用端口", fw.Name, n.Name))
				continue
			}
			port := available[0]
			if preferredPort != nil {
				for _, p := range available {
					if p == *preferredPort {
						port = p
						break
					}
				}
			}
			fp := &model.ForwardPort{ForwardID: fw.ID, NodeID: nodeID, Port: port}
			if err := s.Port.InsertOne(fp); err != nil {
				*failures = append(*failures, fmt.Sprintf("转发[%s]在节点[%s]保存端口失败: %s", fw.Name, n.Name, err.Error()))
				continue
			}
			msg := fwSvc.addOrUpdateService(base, limiter, n, fw, fp, tunnel, "AddService")
			if !gost.IsOK(msg) {
				*failures = append(*failures, fmt.Sprintf("转发[%s]在节点[%s]创建失败: %s", fw.Name, n.Name, gost.NormalizeOK(msg)))
			} else if fw.Status == 0 {
				_ = s.Hub.SendMsg(nodeID, gost.PauseResumePayload(base), "PauseService")
			}
		}
	}
}

// ---- delete ----

func (s *TunnelService) Delete(id int64) error {
	if id == 0 {
		return fmt.Errorf("隧道ID不能为空")
	}
	tunnel, err := s.Tunnel.GetByID(id)
	if err != nil {
		return err
	}
	if tunnel == nil {
		return fmt.Errorf("隧道不存在")
	}

	// 禁止级联：有转发或用户权限时拒绝删除，需先手动清理
	fwCount, err := s.Forward.CountByTunnelID(id)
	if err != nil {
		return err
	}
	utCount, err := s.UserTunn.CountByTunnelID(id)
	if err != nil {
		return err
	}
	if fwCount > 0 || utCount > 0 {
		parts := make([]string, 0, 2)
		if fwCount > 0 {
			parts = append(parts, fmt.Sprintf("%d 条转发", fwCount))
		}
		if utCount > 0 {
			parts = append(parts, fmt.Sprintf("%d 条用户权限", utCount))
		}
		return fmt.Errorf("无法删除隧道：仍有 %s，请先删除转发并取消用户隧道分配后再删隧道", strings.Join(parts, "、"))
	}

	// 无业务关联时，仅清 gost 拓扑 + chain_tunnel + tunnel
	chainTunnels, _ := s.Chain.ListByTunnelID(id)
	for _, ct := range chainTunnels {
		switch repo.ChainTypeOf(ct) {
		case 1:
			deleteChains(s.Hub, ct.NodeID, gost.ChainName(ct.TunnelID))
		case 2:
			deleteChains(s.Hub, ct.NodeID, gost.ChainName(ct.TunnelID))
			deleteService(s.Hub, ct.NodeID, []string{gost.TunnelTLS(ct.TunnelID)})
		default:
			deleteService(s.Hub, ct.NodeID, []string{gost.TunnelTLS(ct.TunnelID)})
		}
	}
	_ = s.Chain.DeleteByTunnelID(id)
	return s.Tunnel.Delete(id)
}

// ---- mine (userTunnel) ----

func (s *TunnelService) Mine(userID int64, roleID int) ([]model.Tunnel, error) {
	if roleID == 0 {
		list, err := s.Tunnel.ListEnabled()
		if err != nil {
			return nil, err
		}
		if list == nil {
			list = []model.Tunnel{}
		}
		return list, nil
	}
	uts, err := s.UserTunn.ListByUserID(int(userID))
	if err != nil {
		return nil, err
	}
	if len(uts) == 0 {
		return []model.Tunnel{}, nil
	}
	ids := make([]int64, 0, len(uts))
	for _, ut := range uts {
		ids = append(ids, int64(ut.TunnelID))
	}
	list, err := s.Tunnel.ListByIDs(ids)
	if err != nil {
		return nil, err
	}
	if list == nil {
		list = []model.Tunnel{}
	}
	return list, nil
}

// ---- diagnose ----

func (s *TunnelService) Diagnose(tunnelID int64) (map[string]any, error) {
	tunnel, err := s.Tunnel.GetByID(tunnelID)
	if err != nil {
		return nil, err
	}
	if tunnel == nil {
		return nil, fmt.Errorf("隧道不存在")
	}
	chainTunnels, err := s.Chain.ListByTunnelID(tunnelID)
	if err != nil {
		return nil, err
	}
	if len(chainTunnels) == 0 {
		return nil, fmt.Errorf("隧道配置不完整")
	}
	entries, chains, outs := splitTopology(chainTunnels)
	results := make([]DiagnosisResult, 0)
	fwSvc := NewForwardService(s.DB, s.Hub)

	if tunnel.Type == 1 {
		for _, inNode := range entries {
			n, _ := s.Node.GetByID(inNode.NodeID)
			if n == nil {
				continue
			}
			r := fwSvc.performTcpPing(n, "www.google.com", 443, "入口("+n.Name+")->外网")
			ft := 1
			r.FromChainType = &ft
			results = append(results, r)
		}
	} else if tunnel.Type == 2 {
		for _, inNode := range entries {
			from, _ := s.Node.GetByID(inNode.NodeID)
			if from == nil {
				continue
			}
			if len(chains) > 0 {
				for _, first := range chains[0] {
					to, _ := s.Node.GetByID(first.NodeID)
					if to == nil {
						continue
					}
					port := 0
					if first.Port != nil {
						port = *first.Port
					}
					r := fwSvc.performTcpPing(from, to.ServerIP, port,
						fmt.Sprintf("入口(%s)->第1跳(%s)", from.Name, to.Name))
					ft, tt := 1, 2
					r.FromChainType = &ft
					r.ToChainType = &tt
					r.ToInx = first.Inx
					results = append(results, r)
				}
			} else {
				// 根据入口的 ExitNodeIDs 过滤出口节点
				targetOuts := filterExitsByEntry(outs, inNode)
				for _, out := range targetOuts {
					to, _ := s.Node.GetByID(out.NodeID)
					if to == nil {
						continue
					}
					port := 0
					if out.Port != nil {
						port = *out.Port
					}
					r := fwSvc.performTcpPing(from, to.ServerIP, port,
						fmt.Sprintf("入口(%s)->出口(%s)", from.Name, to.Name))
					ft, tt := 1, 3
					r.FromChainType = &ft
					r.ToChainType = &tt
					results = append(results, r)
				}
			}
		}

		for i := range chains {
			for _, current := range chains[i] {
				from, _ := s.Node.GetByID(current.NodeID)
				if from == nil {
					continue
				}
				if i+1 < len(chains) {
					for _, next := range chains[i+1] {
						to, _ := s.Node.GetByID(next.NodeID)
						if to == nil {
							continue
						}
						port := 0
						if next.Port != nil {
							port = *next.Port
						}
						r := fwSvc.performTcpPing(from, to.ServerIP, port,
							fmt.Sprintf("第%d跳(%s)->第%d跳(%s)", i+1, from.Name, i+2, to.Name))
						ft, tt := 2, 2
						r.FromChainType = &ft
						r.FromInx = current.Inx
						r.ToChainType = &tt
						r.ToInx = next.Inx
						results = append(results, r)
					}
				} else {
					for _, out := range outs {
						to, _ := s.Node.GetByID(out.NodeID)
						if to == nil {
							continue
						}
						port := 0
						if out.Port != nil {
							port = *out.Port
						}
						r := fwSvc.performTcpPing(from, to.ServerIP, port,
							fmt.Sprintf("第%d跳(%s)->出口(%s)", i+1, from.Name, to.Name))
						ft, tt := 2, 3
						r.FromChainType = &ft
						r.FromInx = current.Inx
						r.ToChainType = &tt
						results = append(results, r)
					}
				}
			}
		}

		for _, out := range outs {
			n, _ := s.Node.GetByID(out.NodeID)
			if n == nil {
				continue
			}
			r := fwSvc.performTcpPing(n, "www.google.com", 443, "出口("+n.Name+")->外网")
			ft := 3
			r.FromChainType = &ft
			results = append(results, r)
		}
	}

	typeLabel := "端口转发"
	if tunnel.Type == 2 {
		typeLabel = "隧道转发"
	}
	return map[string]any{
		"tunnelId":   tunnelID,
		"tunnelName": tunnel.Name,
		"tunnelType": typeLabel,
		"results":    results,
		"timestamp":  time.Now().UnixMilli(),
	}, nil
}

// ---- ports ----

func (s *TunnelService) GetNodePort(nodeID int64) (int, error) {
	ports, err := s.GetAvailablePorts(nodeID)
	if err != nil {
		return 0, err
	}
	if len(ports) == 0 {
		return 0, fmt.Errorf("节点端口已满，无可用端口")
	}
	return ports[0], nil
}

func (s *TunnelService) GetAvailablePorts(nodeID int64) ([]int, error) {
	node, err := s.Node.GetByID(nodeID)
	if err != nil {
		return nil, err
	}
	if node == nil {
		return nil, fmt.Errorf("节点不存在")
	}
	used := make(map[int]struct{})
	cts, err := s.Chain.ListByNodeID(nodeID)
	if err != nil {
		return nil, err
	}
	for _, ct := range cts {
		if ct.Port != nil {
			used[*ct.Port] = struct{}{}
		}
	}
	fps, err := s.Port.ListByNodeID(nodeID)
	if err != nil {
		return nil, err
	}
	for _, fp := range fps {
		used[fp.Port] = struct{}{}
	}
	parsed, err := repo.ParsePorts(node.Port)
	if err != nil {
		return nil, err
	}
	out := make([]int, 0)
	for _, p := range parsed {
		if _, ok := used[p]; !ok {
			out = append(out, p)
		}
	}
	return out, nil
}

// ---- gost helpers (create path: Add*) ----

func (s *TunnelService) addChains(nodeID int64, target []model.ChainTunnel, nodes map[int64]*model.Node) ws.GostResult {
	if s.Hub == nil || len(target) == 0 {
		return ws.GostResult{Msg: "节点不在线"}
	}
	tunnelID := target[0].TunnelID
	src := nodes[nodeID]
	iface := ""
	if src != nil && src.InterfaceName != nil {
		iface = *src.InterfaceName
	}
	strategy := ""
	if target[0].Strategy != nil {
		strategy = *target[0].Strategy
	}
	inputs := make([]gost.ChainNodeInput, 0, len(target))
	for _, ct := range target {
		n := nodes[ct.NodeID]
		if n == nil {
			continue
		}
		port := 0
		if ct.Port != nil {
			port = *ct.Port
		}
		proto := ""
		if ct.Protocol != nil {
			proto = *ct.Protocol
		}
		inputs = append(inputs, gost.ChainNodeInput{
			Protocol: proto,
			ServerIP: n.ServerIP,
			Port:     port,
		})
	}
	data := gost.BuildChainData(tunnelID, nodeID, iface, strategy, inputs)
	// 节点未连接时跳过下发，仅保存数据库配置
	if !s.Hub.IsNodeOnline(nodeID) {
		return ws.GostResult{Msg: "OK"}
	}
	res := s.Hub.SendMsg(nodeID, data, "AddChains")
	res.Msg = gost.NormalizeOK(res.Msg)
	return res
}

func (s *TunnelService) addChainService(nodeID int64, ct model.ChainTunnel, nodes map[int64]*model.Node) ws.GostResult {
	if s.Hub == nil {
		return ws.GostResult{Msg: "节点不在线"}
	}
	// 节点未连接时跳过下发，仅保存数据库配置
	if !s.Hub.IsNodeOnline(nodeID) {
		return ws.GostResult{Msg: "OK"}
	}
	n := nodes[ct.NodeID]
	if n == nil {
		return ws.GostResult{Msg: "节点不存在"}
	}
	src := nodes[nodeID]
	iface := ""
	srcHas := false
	if src != nil && src.InterfaceName != nil && *src.InterfaceName != "" {
		iface = *src.InterfaceName
		srcHas = true
	}
	proto := "tls"
	if ct.Protocol != nil && *ct.Protocol != "" {
		proto = *ct.Protocol
	}
	port := 0
	if ct.Port != nil {
		port = *ct.Port
	}
	tcpListen := n.TCPListenAddr
	if tcpListen == "" {
		tcpListen = "[::]"
	}
	services := gost.BuildChainService(ct.TunnelID, repo.ChainTypeOf(ct), proto, tcpListen, port, iface, srcHas)
	res := s.Hub.SendMsg(nodeID, services, "AddService")
	res.Msg = gost.NormalizeOK(res.Msg)
	return res
}

// ---- topology helpers ----

func splitTopology(cts []model.ChainTunnel) (entries []model.ChainTunnel, chains [][]model.ChainTunnel, outs []model.ChainTunnel) {
	entries = []model.ChainTunnel{}
	outs = []model.ChainTunnel{}
	groups := make(map[int][]model.ChainTunnel)
	for _, ct := range cts {
		switch repo.ChainTypeOf(ct) {
		case 1:
			entries = append(entries, ct)
		case 2:
			inx := 0
			if ct.Inx != nil {
				inx = *ct.Inx
			}
			groups[inx] = append(groups[inx], ct)
		case 3:
			outs = append(outs, ct)
		}
	}
	keys := make([]int, 0, len(groups))
	for k := range groups {
		keys = append(keys, k)
	}
	sort.Ints(keys)
	chains = make([][]model.ChainTunnel, 0, len(keys))
	for _, k := range keys {
		chains = append(chains, groups[k])
	}
	return
}

func nodeName(nodes map[int64]*model.Node, id int64) string {
	if n := nodes[id]; n != nil {
		return n.Name
	}
	return fmt.Sprintf("%d", id)
}

// filterExitsByEntry 根据入口节点的 ExitNodeIDs 过滤出口节点
// 如果入口没有指定 ExitNodeIDs，则返回所有出口节点
func filterExitsByEntry(outNodes []model.ChainTunnel, entry model.ChainTunnel) []model.ChainTunnel {
	if entry.ExitNodeIDs == nil || *entry.ExitNodeIDs == "" {
		// 没有指定出口绑定，返回所有出口节点
		return outNodes
	}

	var exitIDs []int64
	if err := json.Unmarshal([]byte(*entry.ExitNodeIDs), &exitIDs); err != nil || len(exitIDs) == 0 {
		// 解析失败或为空，返回所有出口节点
		return outNodes
	}

	// 构建 exitIDs 的查找集合
	exitIDSet := make(map[int64]struct{}, len(exitIDs))
	for _, id := range exitIDs {
		exitIDSet[id] = struct{}{}
	}

	// 过滤出口节点
	var filtered []model.ChainTunnel
	for _, node := range outNodes {
		if _, ok := exitIDSet[node.NodeID]; ok {
			filtered = append(filtered, node)
		}
	}

	// 如果过滤后为空，返回所有出口节点（防止配置错误导致无出口）
	if len(filtered) == 0 {
		return outNodes
	}

	return filtered
}
