package service

import (
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
	NodeID int64 `json:"nodeId"`
}

type TunnelCreateReq struct {
	Name         string             `json:"name"`
	Type         int                `json:"type"`
	Flow         int                `json:"flow"`
	TrafficRatio float64            `json:"trafficRatio"`
	InIP         string             `json:"inIp"`
	InNodeID     []ChainTunnelInput `json:"inNodeId"`
}

type TunnelUpdateReq struct {
	ID           int64              `json:"id"`
	Name         string             `json:"name"`
	Flow         int                `json:"flow"`
	TrafficRatio float64            `json:"trafficRatio"`
	InIP         string             `json:"inIp"`
	InNodeID     []ChainTunnelInput `json:"inNodeId"`
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
}

// ---- create ----

func (s *TunnelService) Create(req TunnelCreateReq) error {
	if strings.TrimSpace(req.Name) == "" {
		return fmt.Errorf("隧道名称不能为空")
	}
	if len(req.InNodeID) == 0 {
		return fmt.Errorf("入口节点不能为空")
	}
	if req.Type != 1 {
		return fmt.Errorf("隧道类型不合法")
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

	// 相关节点一次查回（原逐个 GetByID + 后续重复 ListByIDs，N+1）
	for _, in := range req.InNodeID {
		if in.NodeID == 0 {
			return fmt.Errorf("节点不存在")
		}
		nodeIDs = append(nodeIDs, in.NodeID)
	}

	// 节点不重复（原先顺序：各节点存在性校验之后、在线校验之前；
	// 缺节点与重复并存时优先报“节点不存在”，此处保持一致）
	set := make(map[int64]struct{})
	for _, id := range nodeIDs {
		set[id] = struct{}{}
	}
	uniqNodeIDs := make([]int64, 0, len(set))
	for id := range set {
		uniqNodeIDs = append(uniqNodeIDs, id)
	}
	batchNodes, err := s.Node.ListByIDs(uniqNodeIDs)
	if err != nil {
		return err
	}
	if len(batchNodes) != len(set) {
		return fmt.Errorf("节点不存在")
	}
	if len(set) != len(nodeIDs) {
		return fmt.Errorf("节点重复")
	}
	for i := range batchNodes {
		nodes[batchNodes[i].ID] = &batchNodes[i]
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

	// 入口
	for _, in := range req.InNodeID {
		n := nodes[in.NodeID]
		if n == nil {
			return fmt.Errorf("节点不存在")
		}
		ct := repo.NewChainTunnel(0, 1, in.NodeID, nil, nil, nil, nil)
		chainTunnels = append(chainTunnels, ct)
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

	if err := s.Chain.InsertBatch(chainTunnels); err != nil {
		_ = s.Tunnel.Delete(tunnel.ID)
		return err
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
		}
		cts := byTunnel[t.ID]
		for _, ct := range cts {
			if repo.ChainTypeOf(ct) == 1 {
				dto.InNodeID = append(dto.InNodeID, ct)
			}
		}
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
		// 入口节点一次查回（原循环内逐个 GetByID）
		entryNodeIDs := make([]int64, 0)
		for _, ct := range entries {
			if repo.ChainTypeOf(ct) != 1 {
				continue
			}
			entryNodeIDs = append(entryNodeIDs, ct.NodeID)
		}
		entryNodes, err := s.Node.ListByIDs(entryNodeIDs)
		if err != nil {
			return err
		}
		entryNodeMap := make(map[int64]*model.Node, len(entryNodes))
		for i := range entryNodes {
			entryNodeMap[entryNodes[i].ID] = &entryNodes[i]
		}
		var parts []string
		for _, ct := range entries {
			if repo.ChainTypeOf(ct) != 1 {
				continue
			}
			n := entryNodeMap[ct.NodeID]
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

	// 规范化
	var newEntries []model.ChainTunnel
	var nodeIDs []int64

	for _, in := range dto.InNodeID {
		if in.NodeID == 0 {
			return fmt.Errorf("节点不存在")
		}
		newEntries = append(newEntries, repo.NewChainTunnel(tunnelID, 1, in.NodeID, nil, nil, nil, nil))
		nodeIDs = append(nodeIDs, in.NodeID)
	}

	set := make(map[int64]struct{})
	for _, id := range nodeIDs {
		if _, ok := set[id]; ok {
			return fmt.Errorf("节点重复")
		}
		set[id] = struct{}{}
	}

	nodes := make(map[int64]*model.Node)
	uniqNodeIDs := make([]int64, 0, len(set))
	for id := range set {
		uniqNodeIDs = append(uniqNodeIDs, id)
	}
	nodeList, err := s.Node.ListByIDs(uniqNodeIDs)
	if err != nil {
		return err
	}
	if len(nodeList) != len(set) {
		return fmt.Errorf("部分节点不存在")
	}
	for i := range nodeList {
		n := &nodeList[i]
		if n.Status != 1 {
			return fmt.Errorf("节点 %s 不在线，无法下发配置", n.Name)
		}
		nodes[n.ID] = n
	}

	oldRecords, err := s.Chain.ListByTunnelID(tunnelID)
	if err != nil {
		return err
	}

	var failures []string

	// DB 替换
	if err := s.Chain.DeleteByTunnelID(tunnelID); err != nil {
		return err
	}
	allNew := make([]model.ChainTunnel, 0, len(newEntries))
	allNew = append(allNew, newEntries...)
	if err := s.Chain.InsertBatch(allNew); err != nil {
		return err
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
	if len(forwards) == 0 {
		return
	}
	fwSvc := NewForwardService(s.DB, s.Hub)

	// 归属权限、已有端口一次查回（原逐转发各 1 次查询）
	fwdIDs := make([]int64, 0, len(forwards))
	for i := range forwards {
		fwdIDs = append(fwdIDs, forwards[i].ID)
	}
	uts, _ := s.UserTunn.ListByTunnelIDs([]int64{tunnel.ID})
	utByUser := make(map[int]*model.UserTunnel, len(uts))
	for i := range uts {
		utByUser[uts[i].UserID] = &uts[i]
	}
	portsMap, _ := s.Port.ListByForwardIDs(fwdIDs)

	// 新增节点可用端口一次算好（原逐转发 GetAvailablePorts 各 3 次查询）
	addedList := make([]int64, 0, len(added))
	for id := range added {
		addedList = append(addedList, id)
	}
	sort.Slice(addedList, func(i, j int) bool { return addedList[i] < addedList[j] })
	availByNode, availErr := s.batchAvailablePorts(addedList)

	removedList := make([]int64, 0, len(removed))
	for id := range removed {
		removedList = append(removedList, id)
	}
	sort.Slice(removedList, func(i, j int) bool { return removedList[i] < removedList[j] })

	// 本次已分配端口（串行分配时扣减，避免同批多条转发选到同一端口）
	consumed := make(map[int64]map[int]struct{}, len(addedList))

	type addTask struct {
		fwIndex int
		base    string
		limiter *int
		fp      *model.ForwardPort
		node    *model.Node
		paused  bool
	}
	var addTasks []addTask
	// 移除：按节点批量删库，WS 删除并行尽力清理
	if len(removedList) > 0 {
		type delTask struct {
			fwIndex int
			nodeID  int64
			names   []string
		}
		var delTasks []delTask
		for fi := range forwards {
			fw := &forwards[fi]
			ut := utByUser[fw.UserID]
			var utID int64
			if ut != nil {
				utID = int64(ut.ID)
			}
			base := gost.ForwardServiceBase(fw.ID, int64(fw.UserID), utID)
			for _, nodeID := range removedList {
				delTasks = append(delTasks, delTask{fwIndex: fi, nodeID: nodeID,
					names: []string{base + "_tcp", base + "_udp"}})
			}
		}
		_ = parallelEach(len(delTasks), batchConcurrency, func(i int) string {
			t := delTasks[i]
			deleteService(s.Hub, t.nodeID, t.names)
			return ""
		})
		for _, nodeID := range removedList {
			_ = s.Port.DeleteByNodeAndForwardIDs(nodeID, fwdIDs)
		}
	}

	for fi := range forwards {
		fw := &forwards[fi]
		ut := utByUser[fw.UserID]
		utID := int64(0)
		var limiter *int
		if ut != nil {
			utID = int64(ut.ID)
			limiter = ut.SpeedID
		}
		base := gost.ForwardServiceBase(fw.ID, int64(fw.UserID), utID)

		// preferred port
		var preferredPort *int
		existingPorts := portsMap[fw.ID]
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

		for _, nodeID := range addedList {
			n := nodes[nodeID]
			if n == nil {
				n2, _ := s.Node.GetByID(nodeID)
				n = n2
			}
			if n == nil {
				*failures = append(*failures, fmt.Sprintf("转发[%s]节点不存在", fw.Name))
				continue
			}
			available := availByNode[nodeID]
			if msg, bad := availErr[nodeID]; bad {
				*failures = append(*failures, fmt.Sprintf("转发[%s]在节点[%s]分配端口失败: %s", fw.Name, n.Name, msg))
				continue
			}
			if consumed[nodeID] != nil {
				filtered := available[:0]
				for _, p := range available {
					if _, used := consumed[nodeID][p]; !used {
						filtered = append(filtered, p)
					}
				}
				available = filtered
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
			if consumed[nodeID] == nil {
				consumed[nodeID] = make(map[int]struct{})
			}
			consumed[nodeID][port] = struct{}{}
			addTasks = append(addTasks, addTask{fwIndex: fi, base: base, limiter: limiter,
				fp: fp, node: n, paused: fw.Status == 0})
		}
	}

	// 新增服务下发限并发并行（DB 已串行落库，端口无竞争）
	addFails := make([]string, len(addTasks))
	_ = parallelEach(len(addTasks), batchConcurrency, func(i int) string {
		t := addTasks[i]
		fw := &forwards[t.fwIndex]
		msg := fwSvc.addOrUpdateService(t.base, t.limiter, t.node, fw, t.fp, tunnel, "AddService")
		if !gost.IsOK(msg) {
			addFails[i] = fmt.Sprintf("转发[%s]在节点[%s]创建失败: %s", fw.Name, t.node.Name, gost.NormalizeOK(msg))
			return addFails[i]
		}
		if t.paused {
			_ = s.Hub.SendMsg(t.fp.NodeID, gost.PauseResumePayload(t.base), "PauseService")
		}
		return ""
	})
	for _, f := range addFails {
		if f != "" {
			*failures = append(*failures, f)
		}
	}
}

// batchAvailablePorts 一次算出多个节点的可用端口（对齐 GetAvailablePorts 逻辑）
// 返回可用端口映射与计算失败的节点错误文案
func (s *TunnelService) batchAvailablePorts(nodeIDs []int64) (map[int64][]int, map[int64]string) {
	out := make(map[int64][]int, len(nodeIDs))
	failed := make(map[int64]string)
	if len(nodeIDs) == 0 {
		return out, failed
	}
	nodes, err := s.Node.ListByIDs(nodeIDs)
	if err != nil {
		for _, id := range nodeIDs {
			failed[id] = err.Error()
		}
		return out, failed
	}
	nodeMap := make(map[int64]*model.Node, len(nodes))
	for i := range nodes {
		nodeMap[nodes[i].ID] = &nodes[i]
	}
	cts, _ := s.Chain.ListByNodeIDs(nodeIDs)
	usedByChain := make(map[int64]map[int]struct{})
	for _, ct := range cts {
		if ct.Port == nil {
			continue
		}
		if usedByChain[ct.NodeID] == nil {
			usedByChain[ct.NodeID] = make(map[int]struct{})
		}
		usedByChain[ct.NodeID][*ct.Port] = struct{}{}
	}
	fps, _ := s.Port.ListByNodeIDs(nodeIDs)
	usedByForward := make(map[int64]map[int]struct{})
	for _, fp := range fps {
		if usedByForward[fp.NodeID] == nil {
			usedByForward[fp.NodeID] = make(map[int]struct{})
		}
		usedByForward[fp.NodeID][fp.Port] = struct{}{}
	}
	for _, id := range nodeIDs {
		n := nodeMap[id]
		if n == nil {
			failed[id] = "节点不存在"
			continue
		}
		parsed, err := repo.ParsePorts(n.Port)
		if err != nil {
			failed[id] = err.Error()
			continue
		}
		avail := make([]int, 0)
		for _, p := range parsed {
			if _, ok := usedByChain[id][p]; ok {
				continue
			}
			if _, ok := usedByForward[id][p]; ok {
				continue
			}
			avail = append(avail, p)
		}
		out[id] = avail
	}
	return out, failed
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

	// 无业务关联时，仅清 chain_tunnel + tunnel
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
	var entries []model.ChainTunnel
	for _, ct := range chainTunnels {
		if repo.ChainTypeOf(ct) == 1 {
			entries = append(entries, ct)
		}
	}
	results := make([]DiagnosisResult, 0)
	fwSvc := NewForwardService(s.DB, s.Hub)

	// 诊断涉及的节点一次查回（替代循环内逐个 GetByID）
	diagNodeSet := make(map[int64]struct{})
	for _, ct := range entries {
		diagNodeSet[ct.NodeID] = struct{}{}
	}
	diagNodeIDs := make([]int64, 0, len(diagNodeSet))
	for id := range diagNodeSet {
		diagNodeIDs = append(diagNodeIDs, id)
	}
	diagNodes, _ := s.Node.ListByIDs(diagNodeIDs)
	diagNodeMap := make(map[int64]*model.Node, len(diagNodes))
	for i := range diagNodes {
		diagNodeMap[diagNodes[i].ID] = &diagNodes[i]
	}

	// 先按原顺序组装探测任务，再限并发执行 TcpPing（单次最长 10s，串行是主要耗时）
	type pingTask struct {
		node          *model.Node
		targetIP      string
		targetPort    int
		description   string
		fromChainType *int
		fromInx       *int
		toChainType   *int
		toInx         *int
	}
	tasks := make([]pingTask, 0)
	addTask := func(node *model.Node, ip string, port int, desc string, ft, fi, tt, ti *int) {
		tasks = append(tasks, pingTask{node: node, targetIP: ip, targetPort: port, description: desc,
			fromChainType: ft, fromInx: fi, toChainType: tt, toInx: ti})
	}
	intPtr := func(v int) *int { p := v; return &p }

	for _, inNode := range entries {
		n := diagNodeMap[inNode.NodeID]
		if n == nil {
			continue
		}
		addTask(n, "www.google.com", 443, "入口("+n.Name+")->外网", intPtr(1), nil, nil, nil)
	}

	if len(tasks) > 0 {
		pinged := make([]DiagnosisResult, len(tasks))
		_ = parallelEach(len(tasks), 8, func(i int) string {
			t := tasks[i]
			pinged[i] = fwSvc.performTcpPing(t.node, t.targetIP, t.targetPort, t.description)
			return ""
		})
		for i := range tasks {
			r := pinged[i]
			r.FromChainType = tasks[i].fromChainType
			r.FromInx = tasks[i].fromInx
			r.ToChainType = tasks[i].toChainType
			r.ToInx = tasks[i].toInx
			results = append(results, r)
		}
	}

	typeLabel := "端口转发"
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



