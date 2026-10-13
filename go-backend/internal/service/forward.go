package service

import (
	"encoding/json"
	"fmt"
	"math/rand"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/gost"
	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/repo"
	"github.com/Fearless743/flux-panel/go-backend/internal/ws"
	"github.com/jmoiron/sqlx"
)

// 批量操作并发度：受节点 WS 往返（最长 10s）主导，IO 等待期间不占 DB 连接，
// 提高并发可显著缩短批量删除/暂停/恢复/迁移的总耗时
const batchConcurrency = 16

const forwardBytesToGB = 1024 * 1024 * 1024

type ForwardService struct {
	DB       *sqlx.DB
	Hub      *ws.Hub
	Repo     *repo.ForwardRepo
	Port     *repo.ForwardPortRepo
	UserTunn *repo.UserTunnelRepo
}

func NewForwardService(db *sqlx.DB, hub *ws.Hub) *ForwardService {
	return &ForwardService{
		DB:       db,
		Hub:      hub,
		Repo:     repo.NewForwardRepo(db),
		Port:     repo.NewForwardPortRepo(db),
		UserTunn: repo.NewUserTunnelRepo(db),
	}
}

type CurrentUser struct {
	UserID   int64
	RoleID   int
	UserName string
}

type ForwardCreateReq struct {
	Name       string `json:"name"`
	TunnelID   int    `json:"tunnelId"`
	RemoteAddr string `json:"remoteAddr"`
	Strategy   string `json:"strategy"`
	InPort     *int   `json:"inPort"`
}

type ForwardUpdateReq struct {
	ID         int64  `json:"id"`
	UserID     int    `json:"userId"`
	Name       string `json:"name"`
	TunnelID   *int   `json:"tunnelId"`
	RemoteAddr string `json:"remoteAddr"`
	Strategy   string `json:"strategy"`
	InPort     *int   `json:"inPort"`
}

type BatchForwardReq struct {
	IDs      []int64 `json:"ids"`
	TunnelID *int    `json:"tunnelId"`
}

type OrderItem struct {
	ID  int64 `json:"id"`
	Inx int   `json:"inx"`
}

type UpdateOrderReq struct {
	Forwards []OrderItem `json:"forwards"`
}

type userPermissionResult struct {
	err        error
	limiter    *int
	userTunnel *model.UserTunnel
}

type DiagnosisResult struct {
	NodeID        int64   `json:"nodeId"`
	NodeName      string  `json:"nodeName"`
	TargetIP      string  `json:"targetIp"`
	TargetPort    int     `json:"targetPort"`
	Description   string  `json:"description"`
	Success       bool    `json:"success"`
	Message       string  `json:"message"`
	AverageTime   float64 `json:"averageTime"`
	PacketLoss    float64 `json:"packetLoss"`
	Timestamp     int64   `json:"timestamp"`
	FromChainType *int    `json:"fromChainType,omitempty"`
	FromInx       *int    `json:"fromInx,omitempty"`
	ToChainType   *int    `json:"toChainType,omitempty"`
	ToInx         *int    `json:"toInx,omitempty"`
}

// ---- list ----

func (s *ForwardService) List(user CurrentUser) ([]repo.ForwardWithTunnel, error) {
	var list []repo.ForwardWithTunnel
	var err error
	if user.RoleID != 0 {
		list, err = s.Repo.ListByUserID(user.UserID)
	} else {
		list, err = s.Repo.ListAll()
	}
	if err != nil {
		return nil, err
	}
	if list == nil {
		list = []repo.ForwardWithTunnel{}
	}

	if len(list) > 0 {
		if err := s.FillInIPBatch(list); err != nil {
			return nil, err
		}
	}
	return list, nil
}

// FillInIPBatch 批量填充转发列表的 inIp / inPort。
// 原逐条 fillInIP 每条转发 2 次查询 + 每端口 1 次节点查询（2N+M+...）；
// 现固定 3 条语句（隧道/端口/节点各一次），内存 map 组装。预取失败时回落
// 逐条路径，保证错误语义不变。
func (s *ForwardService) FillInIPBatch(list []repo.ForwardWithTunnel) error {
	tunnelIDSet := make(map[int64]struct{}, len(list))
	forwardIDs := make([]int64, 0, len(list))
	for _, f := range list {
		tunnelIDSet[int64(f.TunnelID)] = struct{}{}
		forwardIDs = append(forwardIDs, f.ID)
	}
	tunnelIDs := make([]int64, 0, len(tunnelIDSet))
	for id := range tunnelIDSet {
		tunnelIDs = append(tunnelIDs, id)
	}
	tunnels, err := s.Repo.ListTunnelsByIDs(tunnelIDs)
	if err != nil {
		return s.fillInIPLegacy(list)
	}
	portsMap, err := s.Port.ListByForwardIDs(forwardIDs)
	if err != nil {
		return s.fillInIPLegacy(list)
	}
	nodeIDSet := make(map[int64]struct{})
	for _, fps := range portsMap {
		for _, fp := range fps {
			nodeIDSet[fp.NodeID] = struct{}{}
		}
	}
	nodeIDs := make([]int64, 0, len(nodeIDSet))
	for id := range nodeIDSet {
		nodeIDs = append(nodeIDs, id)
	}
	nodes, err := s.Repo.ListNodesByIDs(nodeIDs)
	if err != nil {
		return s.fillInIPLegacy(list)
	}
	tunnelMap := make(map[int64]*model.Tunnel, len(tunnels))
	for i := range tunnels {
		tunnelMap[tunnels[i].ID] = &tunnels[i]
	}
	nodeMap := make(map[int64]*model.Node, len(nodes))
	for i := range nodes {
		nodeMap[nodes[i].ID] = &nodes[i]
	}
	for i := range list {
		s.fillInIPLocal(&list[i], tunnelMap, portsMap, nodeMap)
	}
	return nil
}

// fillInIPLegacy 逐条回填（预取失败时的回落路径，与原 List 行为一致）
func (s *ForwardService) fillInIPLegacy(list []repo.ForwardWithTunnel) error {
	for i := range list {
		s.fillInIP(&list[i])
	}
	return nil
}

func (s *ForwardService) fillInIPLocal(f *repo.ForwardWithTunnel,
	tunnelMap map[int64]*model.Tunnel,
	portsMap map[int64][]model.ForwardPort,
	nodeMap map[int64]*model.Node,
) {
	tunnel := tunnelMap[int64(f.TunnelID)]
	ports := portsMap[f.ID]
	if tunnel == nil || len(ports) == 0 {
		return
	}

	useTunnelInIP := tunnel.InIP != nil && strings.TrimSpace(*tunnel.InIP) != ""
	ipPortSet := make([]string, 0)
	seen := make(map[string]struct{})

	if useTunnelInIP {
		ipList := make([]string, 0)
		for _, ip := range strings.Split(*tunnel.InIP, ",") {
			ip = strings.TrimSpace(ip)
			if ip != "" {
				ipList = append(ipList, ip)
			}
		}
		// distinct ips
		uniqIP := uniqueStrings(ipList)
		uniqPorts := uniqueInts(portsToInts(ports))
		for _, ip := range uniqIP {
			for _, p := range uniqPorts {
				key := fmt.Sprintf("%s:%d", ip, p)
				if _, ok := seen[key]; !ok {
					seen[key] = struct{}{}
					ipPortSet = append(ipPortSet, key)
				}
			}
		}
		if len(uniqPorts) > 0 {
			p := uniqPorts[0]
			f.InPort = &p
		}
	} else {
		for _, fp := range ports {
			node := nodeMap[fp.NodeID]
			if node == nil || node.ServerIP == "" {
				continue
			}
			key := fmt.Sprintf("%s:%d", node.ServerIP, fp.Port)
			if _, ok := seen[key]; !ok {
				seen[key] = struct{}{}
				ipPortSet = append(ipPortSet, key)
			}
		}
		if len(ports) > 0 {
			p := ports[0].Port
			f.InPort = &p
		}
	}
	if len(ipPortSet) > 0 {
		f.InIP = strings.Join(ipPortSet, ",")
	}
}

func (s *ForwardService) fillInIP(f *repo.ForwardWithTunnel) {
	tunnel, err := s.Repo.GetTunnel(int64(f.TunnelID))
	if err != nil || tunnel == nil {
		return
	}
	ports, err := s.Port.ListByForwardID(f.ID)
	if err != nil || len(ports) == 0 {
		return
	}

	useTunnelInIP := tunnel.InIP != nil && strings.TrimSpace(*tunnel.InIP) != ""
	ipPortSet := make([]string, 0)
	seen := make(map[string]struct{})

	if useTunnelInIP {
		ipList := make([]string, 0)
		for _, ip := range strings.Split(*tunnel.InIP, ",") {
			ip = strings.TrimSpace(ip)
			if ip != "" {
				ipList = append(ipList, ip)
			}
		}
		// distinct ips
		uniqIP := uniqueStrings(ipList)
		uniqPorts := uniqueInts(portsToInts(ports))
		for _, ip := range uniqIP {
			for _, p := range uniqPorts {
				key := fmt.Sprintf("%s:%d", ip, p)
				if _, ok := seen[key]; !ok {
					seen[key] = struct{}{}
					ipPortSet = append(ipPortSet, key)
				}
			}
		}
		if len(uniqPorts) > 0 {
			p := uniqPorts[0]
			f.InPort = &p
		}
	} else {
		for _, fp := range ports {
			node, err := s.Repo.GetNode(fp.NodeID)
			if err != nil || node == nil || node.ServerIP == "" {
				continue
			}
			key := fmt.Sprintf("%s:%d", node.ServerIP, fp.Port)
			if _, ok := seen[key]; !ok {
				seen[key] = struct{}{}
				ipPortSet = append(ipPortSet, key)
			}
		}
		if len(ports) > 0 {
			p := ports[0].Port
			f.InPort = &p
		}
	}
	if len(ipPortSet) > 0 {
		f.InIP = strings.Join(ipPortSet, ",")
	}
}

// ---- create ----

func (s *ForwardService) Create(user CurrentUser, req ForwardCreateReq) error {
	if strings.TrimSpace(req.Name) == "" {
		return fmt.Errorf("转发名称不能为空")
	}
	if req.TunnelID == 0 {
		return fmt.Errorf("隧道ID不能为空")
	}
	if strings.TrimSpace(req.RemoteAddr) == "" {
		return fmt.Errorf("远程地址不能为空")
	}

	tunnel, err := s.Repo.GetTunnel(int64(req.TunnelID))
	if err != nil {
		return err
	}
	if tunnel == nil {
		return fmt.Errorf("隧道不存在")
	}
	if tunnel.Status != 1 {
		return fmt.Errorf("隧道已禁用，无法创建转发")
	}

	perm := s.checkUserPermissions(user, tunnel, nil)
	if perm.err != nil {
		return perm.err
	}

	now := time.Now().UnixMilli()
	forward := &model.Forward{
		UserID:      int(user.UserID),
		UserName:    user.UserName,
		Name:        req.Name,
		TunnelID:    req.TunnelID,
		RemoteAddr:  req.RemoteAddr,
		Strategy:    req.Strategy,
		Status:      1,
		CreatedTime: now,
		UpdatedTime: now,
		Inx:         0,
	}

	chainTunnels, err := s.Repo.ListChainTunnelsByTunnel(tunnel.ID, "1")
	if err != nil {
		return err
	}
	if len(chainTunnels) == 0 {
		return fmt.Errorf("隧道没有入口节点")
	}
	chainTunnels, err = s.getPort(chainTunnels, req.InPort, 0)
	if err != nil {
		return err
	}

	id, err := s.Repo.Insert(forward)
	if err != nil {
		return err
	}
	forward.ID = id

	success := make([]forwardSuccessItem, 0)

	for _, ct := range chainTunnels {
		fp := &model.ForwardPort{
			ForwardID: forward.ID,
			NodeID:    ct.NodeID,
			Port:      derefInt(ct.Port),
		}
		if _, err := s.Port.Insert(fp); err != nil {
			s.rollbackCreate(forward.ID, success)
			return err
		}
		serviceName := s.buildServiceName(forward.ID, int64(forward.UserID), perm.userTunnel)
		node, err := s.Repo.GetNode(ct.NodeID)
		if err != nil {
			s.rollbackCreate(forward.ID, success)
			return err
		}
		if node == nil {
			s.rollbackCreate(forward.ID, success)
			return fmt.Errorf("部分节点不存在")
		}
		msg := s.addOrUpdateService(serviceName, perm.limiter, node, forward, fp, tunnel, "AddService")
		if gost.IsOK(msg) {
			success = append(success, forwardSuccessItem{nodeID: node.ID, name: serviceName})
		} else {
			s.rollbackCreate(forward.ID, success)
			return fmt.Errorf("%s", gost.NormalizeOK(msg))
		}
	}
	return nil
}

type forwardSuccessItem struct {
	nodeID int64
	name   string
}

func (s *ForwardService) rollbackCreate(forwardID int64, items []forwardSuccessItem) {
	_ = s.Repo.Delete(forwardID)
	_ = s.Port.DeleteByForwardID(forwardID)
	parallelEach(len(items), 8, func(i int) string {
		names := []string{items[i].name + "_tcp", items[i].name + "_udp"}
		res := s.Hub.SendMsg(items[i].nodeID, gost.DeleteServicePayload(names), "DeleteService")
		_ = gost.NormalizeOK(res.Msg)
		return ""
	})
}

// ---- update ----

func (s *ForwardService) Update(user CurrentUser, req ForwardUpdateReq) error {
	if req.ID == 0 {
		return fmt.Errorf("ID不能为空")
	}
	if strings.TrimSpace(req.Name) == "" {
		return fmt.Errorf("转发名称不能为空")
	}
	if strings.TrimSpace(req.RemoteAddr) == "" {
		return fmt.Errorf("远程地址不能为空")
	}

	exist, err := s.validateForwardExists(req.ID, user)
	if err != nil {
		return err
	}
	if exist == nil {
		return fmt.Errorf("转发不存在")
	}

	tunnel, err := s.Repo.GetTunnel(int64(exist.TunnelID))
	if err != nil {
		return err
	}
	if tunnel == nil {
		return fmt.Errorf("隧道不存在")
	}

	// 换隧道
	if req.TunnelID != nil && *req.TunnelID != exist.TunnelID {
		return s.changeForwardTunnel(user, exist, tunnel, req)
	}

	perm := s.checkUserPermissions(user, tunnel, nil)
	if perm.err != nil {
		return perm.err
	}

	var userTunnel *model.UserTunnel
	if user.RoleID != 0 {
		userTunnel, err = s.Repo.GetUserTunnel(int(user.UserID), exist.TunnelID)
		if err != nil {
			return err
		}
		if userTunnel == nil {
			return fmt.Errorf("你没有该隧道权限")
		}
	} else {
		userTunnel, _ = s.Repo.GetUserTunnel(exist.UserID, exist.TunnelID)
	}

	exist.RemoteAddr = req.RemoteAddr
	exist.Name = req.Name
	exist.Strategy = req.Strategy
	exist.Status = 1
	exist.UpdatedTime = time.Now().UnixMilli()
	if err := s.Repo.Update(exist); err != nil {
		return err
	}

	chainTunnels, err := s.Repo.ListChainTunnelsByTunnel(tunnel.ID, "1")
	if err != nil {
		return err
	}
	chainTunnels, err = s.getPort(chainTunnels, req.InPort, exist.ID)
	if err != nil {
		return err
	}

	for _, ct := range chainTunnels {
		serviceName := s.buildServiceName(exist.ID, int64(exist.UserID), userTunnel)
		node, err := s.Repo.GetNode(ct.NodeID)
		if err != nil {
			return err
		}
		if node == nil {
			return fmt.Errorf("部分节点不存在")
		}
		fp, err := s.Port.GetByForwardAndNode(exist.ID, node.ID)
		if err != nil {
			return err
		}
		if fp == nil {
			return fmt.Errorf("部分节点不存在1")
		}
		fp.Port = derefInt(ct.Port)
		if err := s.Port.UpdatePort(fp.ID, fp.Port); err != nil {
			return err
		}
		msg := s.addOrUpdateService(serviceName, perm.limiter, node, exist, fp, tunnel, "UpdateService")
		if !gost.IsOK(msg) {
			return fmt.Errorf("%s", gost.NormalizeOK(msg))
		}
	}
	return nil
}

// ---- change tunnel ----

func (s *ForwardService) changeForwardTunnel(user CurrentUser, exist *model.Forward, oldTunnel *model.Tunnel, req ForwardUpdateReq) error {
	newTunnel, err := s.Repo.GetTunnel(int64(*req.TunnelID))
	if err != nil {
		return err
	}
	if newTunnel == nil {
		return fmt.Errorf("目标隧道不存在")
	}
	if newTunnel.Status != 1 {
		return fmt.Errorf("目标隧道已禁用")
	}

	perm := s.checkUserPermissions(user, newTunnel, &exist.ID)
	if perm.err != nil {
		return perm.err
	}

	newUserTunnel, _ := s.Repo.GetUserTunnel(exist.UserID, int(newTunnel.ID))
	owner, _ := s.Repo.GetUser(int64(exist.UserID))
	if owner != nil && owner.RoleID != 0 && newUserTunnel == nil {
		return fmt.Errorf("该转发所属用户没有目标隧道权限")
	}

	newEntryNodes, err := s.Repo.ListChainTunnelsByTunnel(newTunnel.ID, "1")
	if err != nil {
		return err
	}
	if len(newEntryNodes) == 0 {
		return fmt.Errorf("目标隧道没有入口节点")
	}

	// 换隧道时保持原入口端口：优先请求指定，否则取原 forward_port
	inPort, err := s.resolveKeepInPort(req.InPort, exist.ID)
	if err != nil {
		return err
	}
	newEntryNodes, err = s.getPort(newEntryNodes, inPort, exist.ID)
	if err != nil {
		return err
	}

	// 删旧服务（多入口并行）
	oldUserTunnel, _ := s.Repo.GetUserTunnel(exist.UserID, int(oldTunnel.ID))
	oldServiceName := s.buildServiceName(exist.ID, int64(exist.UserID), oldUserTunnel)
	oldEntryNodes, _ := s.Repo.ListChainTunnelsByTunnel(oldTunnel.ID, "1")
	names := []string{oldServiceName + "_tcp", oldServiceName + "_udp"}
	_ = s.forEachChainNodeParallel(oldEntryNodes, func(node *model.Node) error {
		res := s.Hub.SendMsg(node.ID, gost.DeleteServicePayload(names), "DeleteService")
		_ = gost.NormalizeOK(res.Msg)
		return nil
	})
	_ = s.Port.DeleteByForwardID(exist.ID)

	// 更新转发
	exist.TunnelID = *req.TunnelID
	exist.Name = req.Name
	exist.RemoteAddr = req.RemoteAddr
	exist.Strategy = req.Strategy
	exist.Status = 1
	exist.UpdatedTime = time.Now().UnixMilli()
	if err := s.Repo.Update(exist); err != nil {
		return err
	}

	// 新服务：先串行写库，再并行下发 AddService
	newServiceName := s.buildServiceName(exist.ID, int64(exist.UserID), newUserTunnel)
	var limiter *int
	if newUserTunnel != nil {
		limiter = newUserTunnel.SpeedID
	}
	type entryWork struct {
		node *model.Node
		fp   *model.ForwardPort
	}
	works := make([]entryWork, 0, len(newEntryNodes))
	for _, ct := range newEntryNodes {
		node, err := s.Repo.GetNode(ct.NodeID)
		if err != nil {
			return err
		}
		if node == nil {
			return fmt.Errorf("目标隧道部分入口节点不存在")
		}
		fp := &model.ForwardPort{
			ForwardID: exist.ID,
			NodeID:    ct.NodeID,
			Port:      derefInt(ct.Port),
		}
		if _, err := s.Port.Insert(fp); err != nil {
			return err
		}
		works = append(works, entryWork{node: node, fp: fp})
	}
	if len(works) == 1 {
		w := works[0]
		msg := s.addOrUpdateService(newServiceName, limiter, w.node, exist, w.fp, newTunnel, "AddService")
		if !gost.IsOK(msg) {
			return fmt.Errorf("节点[%s]服务创建失败: %s", w.node.Name, gost.NormalizeOK(msg))
		}
		return nil
	}
	errCh := make(chan error, len(works))
	var wg sync.WaitGroup
	for _, w := range works {
		w := w
		wg.Add(1)
		go func() {
			defer wg.Done()
			msg := s.addOrUpdateService(newServiceName, limiter, w.node, exist, w.fp, newTunnel, "AddService")
			if !gost.IsOK(msg) {
				errCh <- fmt.Errorf("节点[%s]服务创建失败: %s", w.node.Name, gost.NormalizeOK(msg))
			}
		}()
	}
	wg.Wait()
	close(errCh)
	for err := range errCh {
		if err != nil {
			return err
		}
	}
	return nil
}

// ---- delete ----

func (s *ForwardService) Delete(user CurrentUser, id int64) error {
	forward, err := s.validateForwardExists(id, user)
	if err != nil {
		return err
	}
	if forward == nil {
		return fmt.Errorf("转发不存在")
	}
	tunnel, err := s.Repo.GetTunnel(int64(forward.TunnelID))
	if err != nil {
		return err
	}
	if tunnel == nil {
		return fmt.Errorf("隧道不存在")
	}

	perm := s.checkUserPermissions(user, tunnel, nil)
	if perm.err != nil {
		return perm.err
	}

	var userTunnel *model.UserTunnel
	if user.RoleID != 0 {
		userTunnel, _ = s.Repo.GetUserTunnel(int(user.UserID), forward.TunnelID)
		if userTunnel == nil {
			return fmt.Errorf("你没有该隧道权限")
		}
	} else {
		userTunnel, _ = s.Repo.GetUserTunnel(forward.UserID, forward.TunnelID)
	}

	chainTunnels, err := s.Repo.ListChainTunnelsByTunnel(tunnel.ID, "1")
	if err != nil {
		return err
	}
	serviceName := s.buildServiceName(forward.ID, int64(forward.UserID), userTunnel)
	names := []string{serviceName + "_tcp", serviceName + "_udp"}
	// 多入口节点并行下发删除（单条失败不阻塞其余）
	_ = s.forEachChainNodeParallel(chainTunnels, func(node *model.Node) error {
		res := s.Hub.SendMsg(node.ID, gost.DeleteServicePayload(names), "DeleteService")
		_ = gost.NormalizeOK(res.Msg)
		return nil
	})
	_ = s.Port.DeleteByForwardID(id)
	return s.Repo.Delete(id)
}

func (s *ForwardService) ForceDelete(user CurrentUser, id int64) error {
	forward, err := s.validateForwardExists(id, user)
	if err != nil {
		return err
	}
	if forward == nil {
		return fmt.Errorf("端口转发不存在")
	}
	if err := s.Repo.Delete(id); err != nil {
		return err
	}
	_ = s.Port.DeleteByForwardID(id)
	return nil
}

// ---- pause / resume ----

func (s *ForwardService) Pause(user CurrentUser, id int64) error {
	return s.changeForwardStatus(user, id, 0, "PauseService")
}

func (s *ForwardService) Resume(user CurrentUser, id int64) error {
	return s.changeForwardStatus(user, id, 1, "ResumeService")
}

func (s *ForwardService) changeForwardStatus(user CurrentUser, id int64, targetStatus int, gostMethod string) error {
	if user.RoleID != 0 {
		u, err := s.Repo.GetUser(user.UserID)
		if err != nil {
			return err
		}
		if u == nil {
			return fmt.Errorf("用户不存在")
		}
		if u.Status == 0 {
			return fmt.Errorf("用户已到期或被禁用")
		}
	}

	forward, err := s.validateForwardExists(id, user)
	if err != nil {
		return err
	}
	if forward == nil {
		return fmt.Errorf("转发不存在")
	}
	tunnel, err := s.Repo.GetTunnel(int64(forward.TunnelID))
	if err != nil {
		return err
	}
	if tunnel == nil {
		return fmt.Errorf("隧道不存在")
	}

	var userTunnel *model.UserTunnel
	if targetStatus == 1 {
		if tunnel.Status != 1 {
			return fmt.Errorf("隧道已禁用，无法恢复服务")
		}
		if user.RoleID != 0 {
			if err := s.checkUserFlowLimits(int(user.UserID), tunnel); err != nil {
				return err
			}
			userTunnel, _ = s.Repo.GetUserTunnel(int(user.UserID), forward.TunnelID)
			if userTunnel == nil {
				return fmt.Errorf("你没有该隧道权限")
			}
			if userTunnel.Status != 1 {
				return fmt.Errorf("隧道被禁用")
			}
		}
	}
	if user.RoleID != 0 && userTunnel == nil {
		userTunnel, _ = s.Repo.GetUserTunnel(int(user.UserID), forward.TunnelID)
		if userTunnel == nil {
			return fmt.Errorf("你没有该隧道权限")
		}
	}
	if userTunnel == nil {
		userTunnel, _ = s.Repo.GetUserTunnel(forward.UserID, forward.TunnelID)
	}

	chainTunnels, err := s.Repo.ListChainTunnelsByTunnel(tunnel.ID, "1")
	if err != nil {
		return err
	}
	serviceName := s.buildServiceName(forward.ID, int64(forward.UserID), userTunnel)
	var limiter *int
	if userTunnel != nil {
		limiter = userTunnel.SpeedID
	}
	if err := s.forEachChainNodeParallel(chainTunnels, func(node *model.Node) error {
		res := s.Hub.SendMsg(node.ID, gost.PauseResumePayload(serviceName), gostMethod)
		if gost.IsOK(res.Msg) {
			return nil
		}
		// 节点上服务不存在：暂停视为已达成；恢复则重建服务（Update 路径同理回落 Add）
		if gost.IsMissing(res.Msg) {
			if targetStatus == 0 {
				return nil
			}
			fp, err := s.Port.GetByForwardAndNode(forward.ID, node.ID)
			if err != nil {
				return err
			}
			if fp == nil {
				return fmt.Errorf("节点 %s 缺少端口记录，无法重建服务", node.Name)
			}
			if msg := s.addOrUpdateService(serviceName, limiter, node, forward, fp, tunnel, "AddService"); !gost.IsOK(msg) {
				return fmt.Errorf("节点 %s 重建服务失败: %s", node.Name, msg)
			}
			return nil
		}
		return fmt.Errorf("%s", gost.NormalizeOK(res.Msg))
	}); err != nil {
		return err
	}
	return s.Repo.UpdateStatus(forward.ID, targetStatus, time.Now().UnixMilli())
}

// ---- diagnose ----

func (s *ForwardService) Diagnose(user CurrentUser, id int64) (map[string]any, error) {
	forward, err := s.validateForwardExists(id, user)
	if err != nil {
		return nil, err
	}
	if forward == nil {
		return nil, fmt.Errorf("转发不存在")
	}
	tunnel, err := s.Repo.GetTunnel(int64(forward.TunnelID))
	if err != nil {
		return nil, err
	}
	if tunnel == nil {
		return nil, fmt.Errorf("隧道不存在")
	}

	chainTunnels, err := s.Repo.ListChainTunnelsByTunnel(tunnel.ID, "")
	if err != nil {
		return nil, err
	}
	if len(chainTunnels) == 0 {
		return nil, fmt.Errorf("隧道配置不完整")
	}

	var inNodes, outNodes []model.ChainTunnel
	chainNodesMap := make(map[int][]model.ChainTunnel)
	for _, ct := range chainTunnels {
		switch ct.ChainType {
		case "1":
			inNodes = append(inNodes, ct)
		case "2":
			inx := 0
			if ct.Inx != nil {
				inx = *ct.Inx
			}
			chainNodesMap[inx] = append(chainNodesMap[inx], ct)
		case "3":
			outNodes = append(outNodes, ct)
		}
	}
	// sorted hops
	var hopKeys []int
	for k := range chainNodesMap {
		hopKeys = append(hopKeys, k)
	}
	// simple sort
	for i := 0; i < len(hopKeys); i++ {
		for j := i + 1; j < len(hopKeys); j++ {
			if hopKeys[j] < hopKeys[i] {
				hopKeys[i], hopKeys[j] = hopKeys[j], hopKeys[i]
			}
		}
	}
	var chainNodesList [][]model.ChainTunnel
	for _, k := range hopKeys {
		chainNodesList = append(chainNodesList, chainNodesMap[k])
	}

	// 诊断涉及的节点一次查回（替代循环内逐个 GetNode）
	nodeIDSet := make(map[int64]struct{})
	for _, ct := range inNodes {
		nodeIDSet[ct.NodeID] = struct{}{}
	}
	for _, hop := range chainNodesList {
		for _, ct := range hop {
			nodeIDSet[ct.NodeID] = struct{}{}
		}
	}
	for _, ct := range outNodes {
		nodeIDSet[ct.NodeID] = struct{}{}
	}
	nodeIDs := make([]int64, 0, len(nodeIDSet))
	for id := range nodeIDSet {
		nodeIDs = append(nodeIDs, id)
	}
	diagNodes, _ := s.Repo.ListNodesByIDs(nodeIDs)
	nodeMap := make(map[int64]*model.Node, len(diagNodes))
	for i := range diagNodes {
		cp := diagNodes[i]
		nodeMap[diagNodes[i].ID] = &cp
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

	results := make([]DiagnosisResult, 0)
	remoteAddresses := strings.Split(forward.RemoteAddr, ",")

	if tunnel.Type == 1 {
		for _, inNode := range inNodes {
			node := nodeMap[inNode.NodeID]
			if node == nil {
				continue
			}
			for _, ra := range remoteAddresses {
				ra = strings.TrimSpace(ra)
				targetIP := extractIPFromAddress(ra)
				targetPort := extractPortFromAddress(ra)
				if targetIP != "" && targetPort != -1 {
					addTask(node, targetIP, targetPort, "入口("+node.Name+")->目标("+ra+")",
						intPtr(1), nil, nil, nil)
				}
			}
		}
	} else if tunnel.Type == 2 {
		// 入口 -> 第一跳/出口
		for _, inNode := range inNodes {
			fromNode := nodeMap[inNode.NodeID]
			if fromNode == nil {
				continue
			}
			if len(chainNodesList) > 0 {
				for _, first := range chainNodesList[0] {
					toNode := nodeMap[first.NodeID]
					if toNode == nil {
						continue
					}
					port := derefInt(first.Port)
					addTask(fromNode, toNode.ServerIP, port, "入口("+fromNode.Name+")->第1跳("+toNode.Name+")",
						intPtr(1), nil, intPtr(2), first.Inx)
				}
			} else if len(outNodes) > 0 {
				for _, out := range outNodes {
					toNode := nodeMap[out.NodeID]
					if toNode == nil {
						continue
					}
					port := derefInt(out.Port)
					addTask(fromNode, toNode.ServerIP, port, "入口("+fromNode.Name+")->出口("+toNode.Name+")",
						intPtr(1), nil, intPtr(3), nil)
				}
			}
		}
		// 链路
		for i, currentHop := range chainNodesList {
			for _, current := range currentHop {
				fromNode := nodeMap[current.NodeID]
				if fromNode == nil {
					continue
				}
				if i+1 < len(chainNodesList) {
					for _, next := range chainNodesList[i+1] {
						toNode := nodeMap[next.NodeID]
						if toNode == nil {
							continue
						}
						port := derefInt(next.Port)
						desc := fmt.Sprintf("第%d跳(%s)->第%d跳(%s)", i+1, fromNode.Name, i+2, toNode.Name)
						addTask(fromNode, toNode.ServerIP, port, desc,
							intPtr(2), current.Inx, intPtr(2), next.Inx)
					}
				} else if len(outNodes) > 0 {
					for _, out := range outNodes {
						toNode := nodeMap[out.NodeID]
						if toNode == nil {
							continue
						}
						port := derefInt(out.Port)
						desc := fmt.Sprintf("第%d跳(%s)->出口(%s)", i+1, fromNode.Name, toNode.Name)
						addTask(fromNode, toNode.ServerIP, port, desc,
							intPtr(2), current.Inx, intPtr(3), nil)
					}
				}
			}
		}
		// 出口 -> 目标
		for _, out := range outNodes {
			node := nodeMap[out.NodeID]
			if node == nil {
				continue
			}
			for _, ra := range remoteAddresses {
				ra = strings.TrimSpace(ra)
				targetIP := extractIPFromAddress(ra)
				targetPort := extractPortFromAddress(ra)
				if targetIP != "" && targetPort != -1 {
					addTask(node, targetIP, targetPort, "出口("+node.Name+")->目标("+ra+")",
						intPtr(3), nil, nil, nil)
				}
			}
		}
	}

	if len(tasks) > 0 {
		pinged := make([]DiagnosisResult, len(tasks))
		_ = parallelEach(len(tasks), 8, func(i int) string {
			t := tasks[i]
			pinged[i] = s.performTcpPing(t.node, t.targetIP, t.targetPort, t.description)
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

	tunnelTypeName := "端口转发"
	if tunnel.Type == 2 {
		tunnelTypeName = "隧道转发"
	}
	return map[string]any{
		"forwardId":   id,
		"forwardName": forward.Name,
		"tunnelType":  tunnelTypeName,
		"results":     results,
		"timestamp":   time.Now().UnixMilli(),
	}, nil
}

func (s *ForwardService) performTcpPing(node *model.Node, targetIP string, port int, description string) DiagnosisResult {
	result := DiagnosisResult{
		NodeID:      node.ID,
		NodeName:    node.Name,
		TargetIP:    targetIP,
		TargetPort:  port,
		Description: description,
		Timestamp:   time.Now().UnixMilli(),
	}
	payload := map[string]any{
		"ip":      targetIP,
		"port":    port,
		"count":   4,
		"timeout": 5000,
	}
	res := s.Hub.SendMsg(node.ID, payload, "TcpPing")
	if gost.IsOK(res.Msg) {
		if len(res.Data) > 0 {
			var data map[string]any
			if err := json.Unmarshal(res.Data, &data); err == nil {
				success, _ := data["success"].(bool)
				result.Success = success
				if success {
					result.Message = "TCP连接成功"
					result.AverageTime = toFloat(data["averageTime"])
					result.PacketLoss = toFloat(data["packetLoss"])
				} else {
					if m, ok := data["errorMessage"].(string); ok {
						result.Message = m
					} else {
						result.Message = "连接失败"
					}
					result.AverageTime = -1
					result.PacketLoss = 100
				}
				return result
			}
		}
		result.Success = true
		result.Message = "TCP连接成功"
		result.AverageTime = 0
		result.PacketLoss = 0
		return result
	}
	result.Success = false
	if res.Msg != "" {
		result.Message = res.Msg
	} else {
		result.Message = "节点无响应"
	}
	result.AverageTime = -1
	result.PacketLoss = 100
	return result
}

// ---- order ----

func (s *ForwardService) UpdateOrder(user CurrentUser, items []OrderItem) error {
	if len(items) == 0 {
		return fmt.Errorf("forwards参数不能为空")
	}
	if user.RoleID != 0 {
		ids := make([]int64, 0, len(items))
		for _, it := range items {
			ids = append(ids, it.ID)
		}
		n, err := s.Repo.CountByIDsAndUser(ids, user.UserID)
		if err != nil {
			return err
		}
		if n != int64(len(ids)) {
			return fmt.Errorf("只能更新自己的转发排序")
		}
	}
	// 单语句 CASE WHEN 批量更新，替代逐条 UpdateInx（N 次往返 → 1 次）
	inxByID := make(map[int64]int, len(items))
	for _, it := range items {
		inxByID[it.ID] = it.Inx
	}
	return s.Repo.UpdateInxBatch(inxByID)
}

// ---- batch ----

// utKey 归属权限映射键："userID:tunnelID"
func utKey(userID, tunnelID int) string {
	return fmt.Sprintf("%d:%d", userID, tunnelID)
}

// batchLoadForwards 一次查询取回 ids 对应的转发并按权限过滤（替代逐条 GetByID）。
// 不存在或无权限的 ID 不在返回 map 中，调用方按原单条语义补错误文案。
func (s *ForwardService) batchLoadForwards(user CurrentUser, ids []int64) (map[int64]*model.Forward, error) {
	list, err := s.Repo.ListByIDs(dedupInt64s(ids))
	if err != nil {
		return nil, err
	}
	out := make(map[int64]*model.Forward, len(list))
	for i := range list {
		f := list[i]
		if user.RoleID != 0 && int64(f.UserID) != user.UserID {
			continue
		}
		cp := f
		out[f.ID] = &cp
	}
	return out, nil
}

// batchTunnelMap 一次查询取回多个隧道
func (s *ForwardService) batchTunnelMap(tunnelIDs []int64) (map[int64]*model.Tunnel, error) {
	list, err := s.Repo.ListTunnelsByIDs(tunnelIDs)
	if err != nil {
		return nil, err
	}
	out := make(map[int64]*model.Tunnel, len(list))
	for i := range list {
		t := list[i]
		out[t.ID] = &t
	}
	return out, nil
}

// distinctTargetTunnelIDs 按顺序收集目标转发涉及的去重隧道 ID
func distinctTargetTunnelIDs(targets []*model.Forward) []int64 {
	seen := make(map[int]struct{}, len(targets))
	out := make([]int64, 0, len(targets))
	for _, f := range targets {
		if _, dup := seen[f.TunnelID]; !dup {
			seen[f.TunnelID] = struct{}{}
			out = append(out, int64(f.TunnelID))
		}
	}
	return out
}

// assembleBatchMsg 按输入顺序拼装批量结果文案（与原 runBatch 格式一致）
func assembleBatchMsg(okVerb string, ids []int64, errByID map[int64]string, success int) string {
	var errs []string
	for _, id := range ids {
		if msg, ok := errByID[id]; ok {
			errs = append(errs, fmt.Sprintf("ID-%d: %s", id, msg))
		}
	}
	msg := fmt.Sprintf("成功%s%d个转发", okVerb, success)
	if len(errs) > 0 {
		msg += fmt.Sprintf("，失败%d个: %s", len(errs), strings.Join(errs, "; "))
	}
	return msg
}

// findForwardPort 在预取的端口列表中找某节点的记录
func findForwardPort(fps []model.ForwardPort, nodeID int64) *model.ForwardPort {
	for i := range fps {
		if fps[i].NodeID == nodeID {
			return &fps[i]
		}
	}
	return nil
}

// runBatch 对 ids 有限并发执行 fn，汇总成功/失败文案（与原先串行语义一致）。
func (s *ForwardService) runBatch(ids []int64, emptyMsg, okVerb string, fn func(id int64) error) (string, bool) {
	if len(ids) == 0 {
		return emptyMsg, false
	}
	workers := batchConcurrency
	if workers > len(ids) {
		workers = len(ids)
	}
	type result struct {
		id  int64
		err error
	}
	jobs := make(chan int64, len(ids))
	out := make(chan result, len(ids))
	var wg sync.WaitGroup
	for i := 0; i < workers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for id := range jobs {
				out <- result{id: id, err: fn(id)}
			}
		}()
	}
	for _, id := range ids {
		jobs <- id
	}
	close(jobs)
	go func() {
		wg.Wait()
		close(out)
	}()

	var errs []string
	success := 0
	// 按提交顺序收集失败信息，便于对照
	errByID := make(map[int64]string, len(ids))
	for r := range out {
		if r.err != nil {
			errByID[r.id] = r.err.Error()
		} else {
			success++
		}
	}
	for _, id := range ids {
		if msg, ok := errByID[id]; ok {
			errs = append(errs, fmt.Sprintf("ID-%d: %s", id, msg))
		}
	}
	msg := fmt.Sprintf("成功%s%d个转发", okVerb, success)
	if len(errs) > 0 {
		msg += fmt.Sprintf("，失败%d个: %s", len(errs), strings.Join(errs, "; "))
	}
	return msg, success > 0
}

func (s *ForwardService) BatchDelete(user CurrentUser, ids []int64) (string, bool) {
	if len(ids) == 0 {
		return "请选择要删除的转发", false
	}
	if msg, ok, fallback := s.batchDeleteFast(user, ids); !fallback {
		return msg, ok
	}
	return s.runBatch(ids, "请选择要删除的转发", "删除", func(id int64) error {
		return s.Delete(user, id)
	})
}

// batchDeleteFast 批量删除快路径：一次预取校验 + 按节点合并下发 + 两条语句删库。
// 原逐条路径每条转发约 9 次 DB 查询 + E 次 WS 往返；快路径固定约 8 次查询、
// 每节点 1 次 WS、下发失败自动回落逐条补发。fallback=true 表示预取阶段 DB
// 出错，调用方回落逐条路径以保留原错误语义。
func (s *ForwardService) batchDeleteFast(user CurrentUser, ids []int64) (string, bool, bool) {
	uniq := dedupInt64s(ids)
	forwards, err := s.batchLoadForwards(user, uniq)
	if err != nil {
		return "", false, true
	}
	errByID := make(map[int64]string, len(uniq))
	var targets []*model.Forward
	for _, id := range uniq {
		f, ok := forwards[id]
		if !ok {
			errByID[id] = "转发不存在"
			continue
		}
		targets = append(targets, f)
	}
	if len(targets) == 0 {
		return assembleBatchMsg("删除", uniq, errByID, 0), false, false
	}

	now := time.Now().UnixMilli()
	// 操作者信息只查一次（原逐条 checkUserPermissions 的用户部分）。
	// 注意单条 Delete 的检查顺序是存在性→隧道→权限：隧道缺失优先于操作者异常，
	// 因此这里只预取、把文案判定放到下面逐条循环里以保持相同优先级。
	var opUser *model.User
	opUserErr := ""
	if user.RoleID != 0 {
		var err error
		opUser, err = s.Repo.GetUser(user.UserID)
		if err != nil {
			return "", false, true
		}
		if opUser == nil {
			opUserErr = "用户不存在"
		} else if opUser.ExpTime > 0 && opUser.ExpTime <= now {
			opUserErr = "当前账号已到期"
		}
	}

	tunnelIDs := distinctTargetTunnelIDs(targets)
	tunnelMap, err := s.batchTunnelMap(tunnelIDs)
	if err != nil {
		return "", false, true
	}
	// 操作者在各隧道的权限 + 配额基数各查一次
	opUTMap := make(map[int64]*model.UserTunnel)
	var userForwardCount int64
	tunnelForwardCount := make(map[int64]int64, len(tunnelIDs))
	if user.RoleID != 0 {
		var opUTs []model.UserTunnel
		if opUTs, err = s.UserTunn.ListByUserAndTunnelIDs(user.UserID, tunnelIDs); err != nil {
			return "", false, true
		}
		for i := range opUTs {
			cp := opUTs[i]
			opUTMap[int64(cp.TunnelID)] = &cp
		}
		if userForwardCount, err = s.Repo.CountByUser(int(user.UserID)); err != nil {
			return "", false, true
		}
		for _, tid := range tunnelIDs {
			var n int64
			if n, err = s.Repo.CountByUserTunnel(int(user.UserID), int(tid), nil); err != nil {
				return "", false, true
			}
			tunnelForwardCount[tid] = n
		}
	}

	var valid []*model.Forward
	for _, f := range targets {
		tid := int64(f.TunnelID)
		tunnel := tunnelMap[tid]
		if tunnel == nil {
			errByID[f.ID] = "隧道不存在"
			continue
		}
		if user.RoleID != 0 {
			if opUserErr != "" {
				errByID[f.ID] = opUserErr
				continue
			}
			ut := opUTMap[tid]
			if ut == nil {
				errByID[f.ID] = "你没有该隧道权限"
				continue
			}
			if ut.Status != 1 {
				errByID[f.ID] = "隧道被禁用"
				continue
			}
			if ut.ExpTime > 0 && ut.ExpTime <= now {
				errByID[f.ID] = "该隧道权限已到期"
				continue
			}
			if !model.IsUnlimitedFlow(opUser.Flow) {
				if opUser.Flow <= 0 || opUser.Flow*forwardBytesToGB <= opUser.InFlow+opUser.OutFlow {
					errByID[f.ID] = "用户总流量已用完"
					continue
				}
			}
			if !model.IsUnlimitedFlow(ut.Flow) {
				if ut.Flow <= 0 || ut.Flow*forwardBytesToGB <= ut.InFlow+ut.OutFlow {
					errByID[f.ID] = "该隧道流量已用完"
					continue
				}
			}
			if !model.IsUnlimitedNum(opUser.Num) && userForwardCount >= int64(opUser.Num) {
				errByID[f.ID] = fmt.Sprintf("用户总转发数量已达上限，当前限制：%d个", opUser.Num)
				continue
			}
			if !model.IsUnlimitedNum(ut.Num) && tunnelForwardCount[tid] >= int64(ut.Num) {
				errByID[f.ID] = fmt.Sprintf("该隧道转发数量已达上限，当前限制：%d个", ut.Num)
				continue
			}
		}
		valid = append(valid, f)
	}
	if len(valid) == 0 {
		return assembleBatchMsg("删除", uniq, errByID, 0), false, false
	}

	validTunnelIDs := distinctTargetTunnelIDs(valid)
	entries, err := s.Repo.ListEntriesByTunnelIDs(validTunnelIDs)
	if err != nil {
		return "", false, true
	}
	entriesByTunnel := make(map[int64][]model.ChainTunnel, len(validTunnelIDs))
	nodeIDSet := make(map[int64]struct{})
	for _, ct := range entries {
		entriesByTunnel[ct.TunnelID] = append(entriesByTunnel[ct.TunnelID], ct)
		nodeIDSet[ct.NodeID] = struct{}{}
	}
	nodeIDs := make([]int64, 0, len(nodeIDSet))
	for id := range nodeIDSet {
		nodeIDs = append(nodeIDs, id)
	}
	nodes, err := s.Repo.ListNodesByIDs(nodeIDs)
	if err != nil {
		return "", false, true
	}
	nodeMap := make(map[int64]*model.Node, len(nodes))
	for i := range nodes {
		cp := nodes[i]
		nodeMap[nodes[i].ID] = &cp
	}
	ownerUTs, err := s.UserTunn.ListByTunnelIDs(validTunnelIDs)
	if err != nil {
		return "", false, true
	}
	ownerUTMap := make(map[string]*model.UserTunnel, len(ownerUTs))
	for i := range ownerUTs {
		cp := ownerUTs[i]
		ownerUTMap[utKey(cp.UserID, cp.TunnelID)] = &cp
	}

	// 校验入口节点存在（原 forEachChainNodeParallel 的“部分节点不存在”），
	// 并把各转发的待删服务名按节点合并
	namesByNode := make(map[int64][]string)
	entriesOf := make(map[int64][]model.ChainTunnel, len(valid))
	var doDelete []*model.Forward
	for _, f := range valid {
		cts := entriesByTunnel[int64(f.TunnelID)]
		missing := false
		for _, ct := range cts {
			if nodeMap[ct.NodeID] == nil {
				missing = true
				break
			}
		}
		if missing {
			errByID[f.ID] = "部分节点不存在"
			continue
		}
		base := s.buildServiceName(f.ID, int64(f.UserID), ownerUTMap[utKey(f.UserID, f.TunnelID)])
		for _, ct := range cts {
			namesByNode[ct.NodeID] = append(namesByNode[ct.NodeID], base+"_tcp", base+"_udp")
		}
		entriesOf[f.ID] = cts
		doDelete = append(doDelete, f)
	}
	if len(doDelete) == 0 {
		return assembleBatchMsg("删除", uniq, errByID, 0), false, false
	}

	// 每节点 1 次 WS 合并删除（原 N×E 次）；整包失败的节点回落逐条补发
	nodeList := make([]int64, 0, len(namesByNode))
	for nid := range namesByNode {
		nodeList = append(nodeList, nid)
	}
	sort.Slice(nodeList, func(i, j int) bool { return nodeList[i] < nodeList[j] })
	nodeFailed := make([]bool, len(nodeList))
	_ = parallelEach(len(nodeList), batchConcurrency, func(i int) string {
		nid := nodeList[i]
		if res := s.Hub.SendMsg(nid, gost.DeleteServicePayload(namesByNode[nid]), "DeleteService"); !gost.IsOK(res.Msg) {
			nodeFailed[i] = true
		}
		return ""
	})
	hasFailedNode := false
	for _, f := range nodeFailed {
		if f {
			hasFailedNode = true
			break
		}
	}
	if hasFailedNode {
		failedSet := make(map[int64]struct{})
		for i, nid := range nodeList {
			if nodeFailed[i] {
				failedSet[nid] = struct{}{}
			}
		}
		_ = parallelEach(len(doDelete), batchConcurrency, func(i int) string {
			f := doDelete[i]
			for _, ct := range entriesOf[f.ID] {
				if _, bad := failedSet[ct.NodeID]; !bad {
					continue
				}
				base := s.buildServiceName(f.ID, int64(f.UserID), ownerUTMap[utKey(f.UserID, f.TunnelID)])
				res := s.Hub.SendMsg(ct.NodeID, gost.DeleteServicePayload([]string{base + "_tcp", base + "_udp"}), "DeleteService")
				_ = gost.NormalizeOK(res.Msg)
			}
			return ""
		})
	}

	delIDs := make([]int64, 0, len(doDelete))
	for _, f := range doDelete {
		delIDs = append(delIDs, f.ID)
	}
	_ = s.Port.DeleteByForwardIDs(delIDs)
	if err := s.Repo.DeleteByIDs(delIDs); err != nil {
		for _, f := range doDelete {
			errByID[f.ID] = err.Error()
		}
		return assembleBatchMsg("删除", uniq, errByID, 0), false, false
	}
	return assembleBatchMsg("删除", uniq, errByID, len(doDelete)), true, false
}

func (s *ForwardService) BatchPause(user CurrentUser, ids []int64) (string, bool) {
	return s.batchChangeStatus(user, ids, 0, "请选择要暂停的转发", "暂停", "PauseService")
}

func (s *ForwardService) BatchResume(user CurrentUser, ids []int64) (string, bool) {
	return s.batchChangeStatus(user, ids, 1, "请选择要恢复的转发", "恢复", "ResumeService")
}

// batchChangeStatus 批量暂停/恢复：分发到快路径，预取失败回落逐条以保留原错误语义
func (s *ForwardService) batchChangeStatus(user CurrentUser, ids []int64, targetStatus int, emptyMsg, okVerb, gostMethod string) (string, bool) {
	if len(ids) == 0 {
		return emptyMsg, false
	}
	if msg, ok, fallback := s.batchChangeStatusFast(user, ids, targetStatus, okVerb, gostMethod); !fallback {
		return msg, ok
	}
	if targetStatus == 0 {
		return s.runBatch(ids, emptyMsg, okVerb, func(id int64) error {
			return s.Pause(user, id)
		})
	}
	return s.runBatch(ids, emptyMsg, okVerb, func(id int64) error {
		return s.Resume(user, id)
	})
}

// batchChangeStatusFast 批量暂停/恢复快路径：一次预取校验 + 限并发 WS + 单语句状态写。
// 原逐条路径每条转发约 8 次 DB 查询 + E 次 WS + 1 次状态写；快路径固定约 7 次
// 查询、WS 限并发 16、成功者一次 UpdateStatusByIDs。错误文案与单条路径逐项对齐。
func (s *ForwardService) batchChangeStatusFast(user CurrentUser, ids []int64, targetStatus int, okVerb, gostMethod string) (string, bool, bool) {
	uniq := dedupInt64s(ids)
	forwards, err := s.batchLoadForwards(user, uniq)
	if err != nil {
		return "", false, true
	}
	now := time.Now().UnixMilli()
	errByID := make(map[int64]string, len(uniq))

	// 非管理员先校验操作者状态（原 changeForwardStatus 首步，优先于“转发不存在”）
	var opUser *model.User
	if user.RoleID != 0 {
		opUser, err = s.Repo.GetUser(user.UserID)
		if err != nil {
			return "", false, true
		}
		if opUser == nil {
			for _, id := range uniq {
				errByID[id] = "用户不存在"
			}
			return assembleBatchMsg(okVerb, uniq, errByID, 0), false, false
		}
		if opUser.Status == 0 {
			for _, id := range uniq {
				errByID[id] = "用户已到期或被禁用"
			}
			return assembleBatchMsg(okVerb, uniq, errByID, 0), false, false
		}
	}
	var targets []*model.Forward
	for _, id := range uniq {
		f, ok := forwards[id]
		if !ok {
			errByID[id] = "转发不存在"
			continue
		}
		targets = append(targets, f)
	}
	if len(targets) == 0 {
		return assembleBatchMsg(okVerb, uniq, errByID, 0), false, false
	}

	tunnelIDs := distinctTargetTunnelIDs(targets)
	tunnelMap, err := s.batchTunnelMap(tunnelIDs)
	if err != nil {
		return "", false, true
	}
	opUTMap := make(map[int64]*model.UserTunnel)
	if user.RoleID != 0 {
		var opUTs []model.UserTunnel
		if opUTs, err = s.UserTunn.ListByUserAndTunnelIDs(user.UserID, tunnelIDs); err != nil {
			return "", false, true
		}
		for i := range opUTs {
			cp := opUTs[i]
			opUTMap[int64(cp.TunnelID)] = &cp
		}
	}

	var valid []*model.Forward
	for _, f := range targets {
		tid := int64(f.TunnelID)
		tunnel := tunnelMap[tid]
		if tunnel == nil {
			errByID[f.ID] = "隧道不存在"
			continue
		}
		if targetStatus == 1 {
			if tunnel.Status != 1 {
				errByID[f.ID] = "隧道已禁用，无法恢复服务"
				continue
			}
			if user.RoleID != 0 {
				// 对齐 checkUserFlowLimits 的检查顺序与文案
				if opUser.ExpTime > 0 && opUser.ExpTime <= now {
					errByID[f.ID] = "当前账号已到期"
					continue
				}
				ut := opUTMap[tid]
				if ut == nil {
					errByID[f.ID] = "你没有该隧道权限"
					continue
				}
				if ut.ExpTime > 0 && ut.ExpTime <= now {
					errByID[f.ID] = "该隧道权限已到期，无法恢复服务"
					continue
				}
				if !model.IsUnlimitedFlow(opUser.Flow) {
					if opUser.Flow*forwardBytesToGB <= opUser.InFlow+opUser.OutFlow {
						errByID[f.ID] = "用户总流量已用完，无法恢复服务"
						continue
					}
				}
				if !model.IsUnlimitedFlow(ut.Flow) {
					if ut.Flow*forwardBytesToGB <= ut.InFlow+ut.OutFlow {
						errByID[f.ID] = "该隧道流量已用完，无法恢复服务"
						continue
					}
				}
				if ut.Status != 1 {
					errByID[f.ID] = "隧道被禁用"
					continue
				}
			}
		} else if user.RoleID != 0 {
			if opUTMap[tid] == nil {
				errByID[f.ID] = "你没有该隧道权限"
				continue
			}
		}
		valid = append(valid, f)
	}
	if len(valid) == 0 {
		return assembleBatchMsg(okVerb, uniq, errByID, 0), false, false
	}

	validTunnelIDs := distinctTargetTunnelIDs(valid)
	entries, err := s.Repo.ListEntriesByTunnelIDs(validTunnelIDs)
	if err != nil {
		return "", false, true
	}
	entriesByTunnel := make(map[int64][]model.ChainTunnel, len(validTunnelIDs))
	nodeIDSet := make(map[int64]struct{})
	for _, ct := range entries {
		entriesByTunnel[ct.TunnelID] = append(entriesByTunnel[ct.TunnelID], ct)
		nodeIDSet[ct.NodeID] = struct{}{}
	}
	nodeIDs := make([]int64, 0, len(nodeIDSet))
	for id := range nodeIDSet {
		nodeIDs = append(nodeIDs, id)
	}
	nodes, err := s.Repo.ListNodesByIDs(nodeIDs)
	if err != nil {
		return "", false, true
	}
	nodeMap := make(map[int64]*model.Node, len(nodes))
	for i := range nodes {
		cp := nodes[i]
		nodeMap[nodes[i].ID] = &cp
	}
	ownerUTs, err := s.UserTunn.ListByTunnelIDs(validTunnelIDs)
	if err != nil {
		return "", false, true
	}
	ownerUTMap := make(map[string]*model.UserTunnel, len(ownerUTs))
	for i := range ownerUTs {
		cp := ownerUTs[i]
		ownerUTMap[utKey(cp.UserID, cp.TunnelID)] = &cp
	}
	// 恢复时节点缺服务需按端口重建，端口一次查回（暂停路径不需要）
	var portsMap map[int64][]model.ForwardPort
	if targetStatus == 1 {
		validIDs := make([]int64, 0, len(valid))
		for _, f := range valid {
			validIDs = append(validIDs, f.ID)
		}
		if portsMap, err = s.Port.ListByForwardIDs(validIDs); err != nil {
			return "", false, true
		}
	}

	type statusWork struct {
		fw      *model.Forward
		tunnel  *model.Tunnel
		entries []model.ChainTunnel
		base    string
		limiter *int
	}
	works := make([]statusWork, 0, len(valid))
	for _, f := range valid {
		tid := int64(f.TunnelID)
		cts := entriesByTunnel[tid]
		missing := false
		for _, ct := range cts {
			if nodeMap[ct.NodeID] == nil {
				missing = true
				break
			}
		}
		if missing {
			errByID[f.ID] = "部分节点不存在"
			continue
		}
		var ut *model.UserTunnel
		if user.RoleID != 0 {
			ut = opUTMap[tid]
		} else {
			ut = ownerUTMap[utKey(f.UserID, f.TunnelID)]
		}
		var limiter *int
		if ut != nil {
			limiter = ut.SpeedID
		}
		works = append(works, statusWork{
			fw:      f,
			tunnel:  tunnelMap[tid],
			entries: cts,
			base:    s.buildServiceName(f.ID, int64(f.UserID), ut),
			limiter: limiter,
		})
	}
	if len(works) == 0 {
		return assembleBatchMsg(okVerb, uniq, errByID, 0), false, false
	}

	// 限并发下发 WS（暂停缺服务视为已达成；恢复缺服务按端口重建）
	failMsgs := make([]string, len(works))
	_ = parallelEach(len(works), batchConcurrency, func(i int) string {
		w := works[i]
		for _, ct := range w.entries {
			node := nodeMap[ct.NodeID]
			res := s.Hub.SendMsg(node.ID, gost.PauseResumePayload(w.base), gostMethod)
			if gost.IsOK(res.Msg) {
				continue
			}
			if gost.IsMissing(res.Msg) {
				if targetStatus == 0 {
					continue
				}
				fp := findForwardPort(portsMap[w.fw.ID], node.ID)
				if fp == nil {
					failMsgs[i] = fmt.Sprintf("节点 %s 缺少端口记录，无法重建服务", node.Name)
					return failMsgs[i]
				}
				if msg := s.addOrUpdateService(w.base, w.limiter, node, w.fw, fp, w.tunnel, "AddService"); !gost.IsOK(msg) {
					failMsgs[i] = fmt.Sprintf("节点 %s 重建服务失败: %s", node.Name, msg)
					return failMsgs[i]
				}
				continue
			}
			failMsgs[i] = gost.NormalizeOK(res.Msg)
			return failMsgs[i]
		}
		return ""
	})
	var succeeded []int64
	for i, w := range works {
		if failMsgs[i] != "" {
			errByID[w.fw.ID] = failMsgs[i]
			continue
		}
		succeeded = append(succeeded, w.fw.ID)
	}
	// 成功者一次状态写（原逐条 UpdateStatus）
	if len(succeeded) > 0 {
		if err := s.Repo.UpdateStatusByIDs(succeeded, targetStatus, now); err != nil {
			for _, id := range succeeded {
				errByID[id] = err.Error()
			}
			return assembleBatchMsg(okVerb, uniq, errByID, 0), false, false
		}
	}
	return assembleBatchMsg(okVerb, uniq, errByID, len(succeeded)), len(succeeded) > 0, false
}

func (s *ForwardService) BatchChangeTunnel(user CurrentUser, ids []int64, tunnelID int) (string, bool) {
	if tunnelID == 0 {
		return "请选择目标隧道", false
	}
	// 预取转发存在性与原入口端口（各 1 条语句，替代 N 次 GetByID + N 次查端口）；
	// 预取失败则回落原逐条语义。Update 内部的隧道/权限/配额校验保持不变。
	forwards, portsMap, prefetchOK := s.prefetchChangeTunnel(user, ids)
	return s.runBatch(ids, "请选择要更改隧道的转发", "迁移", func(id int64) error {
		var exist *model.Forward
		var inPort *int
		if prefetchOK {
			exist = forwards[id]
			if exist == nil {
				return fmt.Errorf("转发不存在或无权限")
			}
			// 沿用原入口端口，避免重分配（对齐 resolveKeepInPort）
			for _, fp := range portsMap[id] {
				if fp.Port > 0 {
					p := fp.Port
					inPort = &p
					break
				}
			}
		} else {
			var err error
			exist, err = s.validateForwardExists(id, user)
			if err != nil {
				return err
			}
			if exist == nil {
				return fmt.Errorf("转发不存在或无权限")
			}
			inPort, err = s.resolveKeepInPort(nil, exist.ID)
			if err != nil {
				return err
			}
		}
		tid := tunnelID
		return s.Update(user, ForwardUpdateReq{
			ID:         id,
			TunnelID:   &tid,
			Name:       exist.Name,
			UserID:     exist.UserID,
			RemoteAddr: exist.RemoteAddr,
			Strategy:   exist.Strategy,
			InPort:     inPort,
		})
	})
}

// prefetchChangeTunnel 一次取回批量改隧道所需的转发与原端口
func (s *ForwardService) prefetchChangeTunnel(user CurrentUser, ids []int64) (map[int64]*model.Forward, map[int64][]model.ForwardPort, bool) {
	forwards, err := s.batchLoadForwards(user, ids)
	if err != nil {
		return nil, nil, false
	}
	portsMap, err := s.Port.ListByForwardIDs(dedupInt64s(ids))
	if err != nil {
		return nil, nil, false
	}
	return forwards, portsMap, true
}

// forEachChainNodeParallel 对入口节点并行执行 fn；任一失败返回首个错误。
func (s *ForwardService) forEachChainNodeParallel(chainTunnels []model.ChainTunnel, fn func(node *model.Node) error) error {
	if len(chainTunnels) == 0 {
		return nil
	}
	if len(chainTunnels) == 1 {
		node, err := s.Repo.GetNode(chainTunnels[0].NodeID)
		if err != nil {
			return err
		}
		if node == nil {
			return fmt.Errorf("部分节点不存在")
		}
		return fn(node)
	}
	errCh := make(chan error, len(chainTunnels))
	var wg sync.WaitGroup
	for _, ct := range chainTunnels {
		ct := ct
		wg.Add(1)
		go func() {
			defer wg.Done()
			node, err := s.Repo.GetNode(ct.NodeID)
			if err != nil {
				errCh <- err
				return
			}
			if node == nil {
				errCh <- fmt.Errorf("部分节点不存在")
				return
			}
			if err := fn(node); err != nil {
				errCh <- err
			}
		}()
	}
	wg.Wait()
	close(errCh)
	for err := range errCh {
		if err != nil {
			return err
		}
	}
	return nil
}

// resolveKeepInPort 换隧道时沿用原入口端口：请求指定优先，否则取该转发已有端口。
// 返回 nil 表示历史上无端口记录（仅此时才允许 getPort 自动分配）。
func (s *ForwardService) resolveKeepInPort(reqPort *int, forwardID int64) (*int, error) {
	if reqPort != nil && *reqPort > 0 {
		p := *reqPort
		return &p, nil
	}
	oldPorts, err := s.Port.ListByForwardID(forwardID)
	if err != nil {
		return nil, err
	}
	if len(oldPorts) == 0 {
		return nil, nil
	}
	// 多入口节点时端口应一致；取首个非 0 端口
	for _, fp := range oldPorts {
		if fp.Port > 0 {
			p := fp.Port
			return &p, nil
		}
	}
	return nil, nil
}

// ---- helpers ----

func (s *ForwardService) validateForwardExists(forwardID int64, user CurrentUser) (*model.Forward, error) {
	f, err := s.Repo.GetByID(forwardID)
	if err != nil {
		return nil, err
	}
	if f == nil {
		return nil, nil
	}
	if user.RoleID != 0 && int64(f.UserID) != user.UserID {
		return nil, nil
	}
	return f, nil
}

func (s *ForwardService) checkUserPermissions(user CurrentUser, tunnel *model.Tunnel, excludeForwardID *int64) userPermissionResult {
	if user.RoleID == 0 {
		return userPermissionResult{}
	}
	userInfo, err := s.Repo.GetUser(user.UserID)
	if err != nil {
		return userPermissionResult{err: err}
	}
	if userInfo == nil {
		return userPermissionResult{err: fmt.Errorf("用户不存在")}
	}
	if userInfo.ExpTime > 0 && userInfo.ExpTime <= time.Now().UnixMilli() {
		return userPermissionResult{err: fmt.Errorf("当前账号已到期")}
	}
	ut, err := s.Repo.GetUserTunnel(int(user.UserID), int(tunnel.ID))
	if err != nil {
		return userPermissionResult{err: err}
	}
	if ut == nil {
		return userPermissionResult{err: fmt.Errorf("你没有该隧道权限")}
	}
	if ut.Status != 1 {
		return userPermissionResult{err: fmt.Errorf("隧道被禁用")}
	}
	if ut.ExpTime > 0 && ut.ExpTime <= time.Now().UnixMilli() {
		return userPermissionResult{err: fmt.Errorf("该隧道权限已到期")}
	}
	// 无限流量直接通过配额检查；有限流量检查已用是否超限
	if !model.IsUnlimitedFlow(userInfo.Flow) {
		if userInfo.Flow <= 0 || userInfo.Flow*forwardBytesToGB <= userInfo.InFlow+userInfo.OutFlow {
			return userPermissionResult{err: fmt.Errorf("用户总流量已用完")}
		}
	}
	if !model.IsUnlimitedFlow(ut.Flow) {
		if ut.Flow <= 0 || ut.Flow*forwardBytesToGB <= ut.InFlow+ut.OutFlow {
			return userPermissionResult{err: fmt.Errorf("该隧道流量已用完")}
		}
	}
	if err := s.checkForwardQuota(int(user.UserID), int(tunnel.ID), ut, userInfo, excludeForwardID); err != nil {
		return userPermissionResult{err: err}
	}
	return userPermissionResult{limiter: ut.SpeedID, userTunnel: ut}
}

func (s *ForwardService) checkForwardQuota(userID, tunnelID int, ut *model.UserTunnel, userInfo *model.User, excludeForwardID *int64) error {
	n, err := s.Repo.CountByUser(userID)
	if err != nil {
		return err
	}
	if !model.IsUnlimitedNum(userInfo.Num) {
		if n >= int64(userInfo.Num) {
			return fmt.Errorf("用户总转发数量已达上限，当前限制：%d个", userInfo.Num)
		}
	}
	tn, err := s.Repo.CountByUserTunnel(userID, tunnelID, excludeForwardID)
	if err != nil {
		return err
	}
	if !model.IsUnlimitedNum(ut.Num) {
		if tn >= int64(ut.Num) {
			return fmt.Errorf("该隧道转发数量已达上限，当前限制：%d个", ut.Num)
		}
	}
	return nil
}

func (s *ForwardService) checkUserFlowLimits(userID int, tunnel *model.Tunnel) error {
	userInfo, err := s.Repo.GetUser(int64(userID))
	if err != nil {
		return err
	}
	if userInfo == nil {
		return fmt.Errorf("用户不存在")
	}
	if userInfo.ExpTime > 0 && userInfo.ExpTime <= time.Now().UnixMilli() {
		return fmt.Errorf("当前账号已到期")
	}
	ut, err := s.Repo.GetUserTunnel(userID, int(tunnel.ID))
	if err != nil {
		return err
	}
	if ut == nil {
		return fmt.Errorf("你没有该隧道权限")
	}
	if ut.ExpTime > 0 && ut.ExpTime <= time.Now().UnixMilli() {
		return fmt.Errorf("该隧道权限已到期，无法恢复服务")
	}
	if !model.IsUnlimitedFlow(userInfo.Flow) {
		if userInfo.Flow*forwardBytesToGB <= userInfo.InFlow+userInfo.OutFlow {
			return fmt.Errorf("用户总流量已用完，无法恢复服务")
		}
	}
	tunnelFlow := ut.InFlow + ut.OutFlow
	if !model.IsUnlimitedFlow(ut.Flow) {
		if ut.Flow*forwardBytesToGB <= tunnelFlow {
			return fmt.Errorf("该隧道流量已用完，无法恢复服务")
		}
	}
	return nil
}

func (s *ForwardService) buildServiceName(forwardID, userID int64, userTunnel *model.UserTunnel) string {
	var utID int64
	if userTunnel != nil {
		utID = int64(userTunnel.ID)
	}
	return gost.ForwardServiceBase(forwardID, userID, utID)
}

func (s *ForwardService) addOrUpdateService(serviceName string, limiter *int, node *model.Node, forward *model.Forward, fp *model.ForwardPort, tunnel *model.Tunnel, meth string) string {
	iface := ""
	if node.InterfaceName != nil {
		iface = *node.InterfaceName
	}
	tcpListen := node.TCPListenAddr
	udpListen := node.UDPListenAddr
	if tcpListen == "" {
		tcpListen = "0.0.0.0"
	}
	if udpListen == "" {
		udpListen = "0.0.0.0"
	}
	services := gost.BuildForwardServices(
		serviceName, limiter, tcpListen, udpListen, fp.Port, iface,
		tunnel.Type, int64(forward.TunnelID), forward.RemoteAddr, forward.Strategy,
	)
	res := s.Hub.SendMsg(node.ID, services, meth)
	// Update 在空节点上必然 not found：回落 Add（弃用本地 gost.json 后的主路径）
	if meth == "UpdateService" && !gost.IsOK(res.Msg) {
		res = s.Hub.SendMsg(node.ID, services, "AddService")
	}
	return gost.NormalizeOK(res.Msg)
}

// getPort 对齐 Java get_port：为入口节点分配端口
func (s *ForwardService) getPort(chainTunnelList []model.ChainTunnel, inPort *int, forwardID int64) ([]model.ChainTunnel, error) {
	if len(chainTunnelList) == 0 {
		return chainTunnelList, nil
	}
	lists := make([][]int, 0, len(chainTunnelList))
	for _, ct := range chainTunnelList {
		ports, err := s.getNodePort(ct.NodeID, forwardID)
		if err != nil {
			return nil, err
		}
		if len(ports) == 0 {
			return nil, fmt.Errorf("暂无可用端口")
		}
		lists = append(lists, ports)
	}

	if inPort != nil {
		for _, ports := range lists {
			if !containsInt(ports, *inPort) {
				return nil, fmt.Errorf("指定端口 %d 不可用（并非所有节点都有此端口）", *inPort)
			}
		}
		for i := range chainTunnelList {
			p := *inPort
			chainTunnelList[i].Port = &p
		}
		return chainTunnelList, nil
	}

	// 交集随机
	inter := make(map[int]struct{})
	for _, p := range lists[0] {
		inter[p] = struct{}{}
	}
	for i := 1; i < len(lists); i++ {
		set := make(map[int]struct{})
		for _, p := range lists[i] {
			if _, ok := inter[p]; ok {
				set[p] = struct{}{}
			}
		}
		inter = set
	}
	if len(inter) > 0 {
		common := make([]int, 0, len(inter))
		for p := range inter {
			common = append(common, p)
		}
		chosen := common[rand.Intn(len(common))]
		for i := range chainTunnelList {
			p := chosen
			chainTunnelList[i].Port = &p
		}
		return chainTunnelList, nil
	}

	// 各自随机
	for i := range chainTunnelList {
		ports := lists[i]
		p := ports[rand.Intn(len(ports))]
		chainTunnelList[i].Port = &p
	}
	return chainTunnelList, nil
}

func (s *ForwardService) getNodePort(nodeID, forwardID int64) ([]int, error) {
	node, err := s.Repo.GetNode(nodeID)
	if err != nil {
		return nil, err
	}
	if node == nil {
		return nil, fmt.Errorf("节点不存在")
	}
	used := make(map[int]struct{})
	// chain_tunnel 占用
	cts, err := s.Repo.ListChainTunnelsByNode(nodeID)
	if err != nil {
		return nil, err
	}
	for _, ct := range cts {
		if ct.Port != nil {
			used[*ct.Port] = struct{}{}
		}
	}
	// forward_port 占用（排除自身）
	fps, err := s.Port.ListByNodeExcludeForward(nodeID, forwardID)
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

// ---- small utils ----

func derefInt(p *int) int {
	if p == nil {
		return 0
	}
	return *p
}

func containsInt(list []int, v int) bool {
	for _, x := range list {
		if x == v {
			return true
		}
	}
	return false
}

func portsToInts(ports []model.ForwardPort) []int {
	out := make([]int, 0, len(ports))
	for _, p := range ports {
		out = append(out, p.Port)
	}
	return out
}

func uniqueInts(in []int) []int {
	seen := make(map[int]struct{})
	out := make([]int, 0, len(in))
	for _, v := range in {
		if _, ok := seen[v]; !ok {
			seen[v] = struct{}{}
			out = append(out, v)
		}
	}
	return out
}

func uniqueStrings(in []string) []string {
	seen := make(map[string]struct{})
	out := make([]string, 0, len(in))
	for _, v := range in {
		if _, ok := seen[v]; !ok {
			seen[v] = struct{}{}
			out = append(out, v)
		}
	}
	return out
}

func extractIPFromAddress(address string) string {
	address = strings.TrimSpace(address)
	if address == "" {
		return ""
	}
	if strings.HasPrefix(address, "[") {
		closeBracket := strings.Index(address, "]")
		if closeBracket > 1 {
			return address[1:closeBracket]
		}
	}
	lastColon := strings.LastIndex(address, ":")
	if lastColon > 0 {
		return address[:lastColon]
	}
	return address
}

func extractPortFromAddress(address string) int {
	address = strings.TrimSpace(address)
	if address == "" {
		return -1
	}
	if strings.HasPrefix(address, "[") {
		closeBracket := strings.Index(address, "]")
		if closeBracket > 1 && closeBracket+1 < len(address) && address[closeBracket+1] == ':' {
			portStr := address[closeBracket+2:]
			var p int
			if _, err := fmt.Sscanf(portStr, "%d", &p); err == nil {
				return p
			}
			return -1
		}
	}
	lastColon := strings.LastIndex(address, ":")
	if lastColon > 0 && lastColon+1 < len(address) {
		portStr := address[lastColon+1:]
		var p int
		if _, err := fmt.Sscanf(portStr, "%d", &p); err == nil {
			return p
		}
	}
	return -1
}

func toFloat(v any) float64 {
	switch t := v.(type) {
	case float64:
		return t
	case float32:
		return float64(t)
	case int:
		return float64(t)
	case int64:
		return float64(t)
	case json.Number:
		f, _ := t.Float64()
		return f
	default:
		return 0
	}
}
