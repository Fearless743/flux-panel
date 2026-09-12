package service

import (
	"encoding/json"
	"fmt"
	"math/rand"
	"strings"
	"sync"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/gost"
	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/repo"
	"github.com/Fearless743/flux-panel/go-backend/internal/ws"
	"github.com/jmoiron/sqlx"
)

// 批量操作并发度：受节点 WS 往返（最长 10s）主导，并发可显著缩短总耗时
const batchConcurrency = 8

const forwardBytesToGB = 1024 * 1024 * 1024

type ForwardService struct {
	DB   *sqlx.DB
	Hub  *ws.Hub
	Repo *repo.ForwardRepo
	Port *repo.ForwardPortRepo
}

func NewForwardService(db *sqlx.DB, hub *ws.Hub) *ForwardService {
	return &ForwardService{
		DB:   db,
		Hub:  hub,
		Repo: repo.NewForwardRepo(db),
		Port: repo.NewForwardPortRepo(db),
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

// FillInIPBatch 批量填充转发列表的 inIp / inPort 字段（单次 N+1 优化）
// 一次查询取所有 tunnel、所有 forward_port、所有 node，构建内存 map 后 O(1) 查找
func (s *ForwardService) FillInIPBatch(list []repo.ForwardWithTunnel) error {
	// 收集所有唯一的 tunnel_id 和 node_id
	tunnelIDs := make(map[int64]struct{})
	nodeIDs := make(map[int64]struct{})
	forwardIDs := make([]int64, 0, len(list))
	for _, f := range list {
		forwardIDs = append(forwardIDs, f.ID)
		tunnelIDs[int64(f.TunnelID)] = struct{}{}
	}
	// 先查 forward_port 获取 node_id 集合
	portsMap, err := s.Port.ListByForwardIDs(forwardIDs)
	if err != nil {
		return err
	}
	for _, fps := range portsMap {
		for _, fp := range fps {
			nodeIDs[fp.NodeID] = struct{}{}
		}
	}
	// 批量查 tunnel
	tunnelSlice := make([]int64, 0, len(tunnelIDs))
	for id := range tunnelIDs {
		tunnelSlice = append(tunnelSlice, id)
	}
	tunnels, err := s.Repo.ListTunnelsByIDs(tunnelSlice)
	if err != nil {
		return err
	}
	tunnelMap := make(map[int64]*model.Tunnel, len(tunnels))
	for i := range tunnels {
		tunnelMap[tunnels[i].ID] = &tunnels[i]
	}
	// 批量查 node
	nodeSlice := make([]int64, 0, len(nodeIDs))
	for id := range nodeIDs {
		nodeSlice = append(nodeSlice, id)
	}
	nodes, err := s.Repo.ListNodesByIDs(nodeSlice)
	if err != nil {
		return err
	}
	nodeMap := make(map[int64]*model.Node, len(nodes))
	for i := range nodes {
		nodeMap[nodes[i].ID] = &nodes[i]
	}
	// 填充每条转发
	for i := range list {
		s.fillInIPLocal(&list[i], tunnelMap, portsMap, nodeMap)
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
	add := func(s string) {
		if _, ok := seen[s]; !ok {
			seen[s] = struct{}{}
			ipPortSet = append(ipPortSet, s)
		}
	}

	if useTunnelInIP {
		ipList := make([]string, 0)
		for _, ip := range strings.Split(*tunnel.InIP, ",") {
			ip = strings.TrimSpace(ip)
			if ip != "" {
				ipList = append(ipList, ip)
			}
		}
		uniqIP := uniqueStrings(ipList)
		uniqPorts := uniqueInts(portsToInts(ports))
		for _, ip := range uniqIP {
			for _, p := range uniqPorts {
				add(fmt.Sprintf("%s:%d", ip, p))
			}
		}
		if len(uniqPorts) > 0 {
			p := uniqPorts[0]
			f.InPort = &p
		}
	} else {
		for _, fp := range ports {
			n := nodeMap[fp.NodeID]
			if n == nil || n.GetEffectiveIP() == "" {
				continue
			}
			add(fmt.Sprintf("%s:%d", n.GetEffectiveIP(), fp.Port))
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
	for _, it := range items {
		names := []string{it.name + "_tcp", it.name + "_udp"}
		res := s.Hub.SendMsg(it.nodeID, gost.DeleteServicePayload(names), "DeleteService")
		_ = gost.NormalizeOK(res.Msg)
	}
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
	if err := s.forEachChainNodeParallel(chainTunnels, func(node *model.Node) error {
		res := s.Hub.SendMsg(node.ID, gost.PauseResumePayload(serviceName), gostMethod)
		if !gost.IsOK(res.Msg) {
			return fmt.Errorf("%s", gost.NormalizeOK(res.Msg))
		}
		return nil
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

	results := make([]DiagnosisResult, 0)
	remoteAddresses := strings.Split(forward.RemoteAddr, ",")

	if tunnel.Type == 1 {
		for _, inNode := range inNodes {
			node, _ := s.Repo.GetNode(inNode.NodeID)
			if node == nil {
				continue
			}
			for _, ra := range remoteAddresses {
				ra = strings.TrimSpace(ra)
				targetIP := extractIPFromAddress(ra)
				targetPort := extractPortFromAddress(ra)
				if targetIP != "" && targetPort != -1 {
					r := s.performTcpPing(node, targetIP, targetPort, "入口("+node.Name+")->目标("+ra+")")
					ft := 1
					r.FromChainType = &ft
					results = append(results, r)
				}
			}
		}
	} else if tunnel.Type == 2 {
		// 入口 -> 第一跳/出口
		for _, inNode := range inNodes {
			fromNode, _ := s.Repo.GetNode(inNode.NodeID)
			if fromNode == nil {
				continue
			}
			if len(chainNodesList) > 0 {
				for _, first := range chainNodesList[0] {
					toNode, _ := s.Repo.GetNode(first.NodeID)
					if toNode == nil {
						continue
					}
					port := derefInt(first.Port)
					r := s.performTcpPing(fromNode, toNode.ServerIP, port, "入口("+fromNode.Name+")->第1跳("+toNode.Name+")")
					ft, tt := 1, 2
					r.FromChainType = &ft
					r.ToChainType = &tt
					r.ToInx = first.Inx
					results = append(results, r)
				}
			} else if len(outNodes) > 0 {
				for _, out := range outNodes {
					toNode, _ := s.Repo.GetNode(out.NodeID)
					if toNode == nil {
						continue
					}
					port := derefInt(out.Port)
					r := s.performTcpPing(fromNode, toNode.ServerIP, port, "入口("+fromNode.Name+")->出口("+toNode.Name+")")
					ft, tt := 1, 3
					r.FromChainType = &ft
					r.ToChainType = &tt
					results = append(results, r)
				}
			}
		}
		// 链路
		for i, currentHop := range chainNodesList {
			for _, current := range currentHop {
				fromNode, _ := s.Repo.GetNode(current.NodeID)
				if fromNode == nil {
					continue
				}
				if i+1 < len(chainNodesList) {
					for _, next := range chainNodesList[i+1] {
						toNode, _ := s.Repo.GetNode(next.NodeID)
						if toNode == nil {
							continue
						}
						port := derefInt(next.Port)
						desc := fmt.Sprintf("第%d跳(%s)->第%d跳(%s)", i+1, fromNode.Name, i+2, toNode.Name)
						r := s.performTcpPing(fromNode, toNode.ServerIP, port, desc)
						ft, tt := 2, 2
						r.FromChainType = &ft
						r.FromInx = current.Inx
						r.ToChainType = &tt
						r.ToInx = next.Inx
						results = append(results, r)
					}
				} else if len(outNodes) > 0 {
					for _, out := range outNodes {
						toNode, _ := s.Repo.GetNode(out.NodeID)
						if toNode == nil {
							continue
						}
						port := derefInt(out.Port)
						desc := fmt.Sprintf("第%d跳(%s)->出口(%s)", i+1, fromNode.Name, toNode.Name)
						r := s.performTcpPing(fromNode, toNode.ServerIP, port, desc)
						ft, tt := 2, 3
						r.FromChainType = &ft
						r.FromInx = current.Inx
						r.ToChainType = &tt
						results = append(results, r)
					}
				}
			}
		}
		// 出口 -> 目标
		for _, out := range outNodes {
			node, _ := s.Repo.GetNode(out.NodeID)
			if node == nil {
				continue
			}
			for _, ra := range remoteAddresses {
				ra = strings.TrimSpace(ra)
				targetIP := extractIPFromAddress(ra)
				targetPort := extractPortFromAddress(ra)
				if targetIP != "" && targetPort != -1 {
					r := s.performTcpPing(node, targetIP, targetPort, "出口("+node.Name+")->目标("+ra+")")
					ft := 3
					r.FromChainType = &ft
					results = append(results, r)
				}
			}
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
	for _, it := range items {
		if err := s.Repo.UpdateInx(it.ID, it.Inx); err != nil {
			return err
		}
	}
	return nil
}

// ---- batch ----

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
	return s.runBatch(ids, "请选择要删除的转发", "删除", func(id int64) error {
		return s.Delete(user, id)
	})
}

func (s *ForwardService) BatchPause(user CurrentUser, ids []int64) (string, bool) {
	return s.runBatch(ids, "请选择要暂停的转发", "暂停", func(id int64) error {
		return s.Pause(user, id)
	})
}

func (s *ForwardService) BatchResume(user CurrentUser, ids []int64) (string, bool) {
	return s.runBatch(ids, "请选择要恢复的转发", "恢复", func(id int64) error {
		return s.Resume(user, id)
	})
}

func (s *ForwardService) BatchChangeTunnel(user CurrentUser, ids []int64, tunnelID int) (string, bool) {
	if tunnelID == 0 {
		return "请选择目标隧道", false
	}
	return s.runBatch(ids, "请选择要更改隧道的转发", "迁移", func(id int64) error {
		exist, err := s.validateForwardExists(id, user)
		if err != nil {
			return err
		}
		if exist == nil {
			return fmt.Errorf("转发不存在或无权限")
		}
		tid := tunnelID
		// 显式带上原入口端口，保证批量改隧道不重分配
		inPort, err := s.resolveKeepInPort(nil, exist.ID)
		if err != nil {
			return err
		}
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
	if userInfo.Flow <= 0 {
		return userPermissionResult{err: fmt.Errorf("用户总流量已用完")}
	}
	if ut.Flow <= 0 {
		return userPermissionResult{err: fmt.Errorf("该隧道流量已用完")}
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
	if n >= int64(userInfo.Num) {
		return fmt.Errorf("用户总转发数量已达上限，当前限制：%d个", userInfo.Num)
	}
	tn, err := s.Repo.CountByUserTunnel(userID, tunnelID, excludeForwardID)
	if err != nil {
		return err
	}
	if tn >= int64(ut.Num) {
		return fmt.Errorf("该隧道转发数量已达上限，当前限制：%d个", ut.Num)
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
	if userInfo.Flow*forwardBytesToGB <= userInfo.InFlow+userInfo.OutFlow {
		return fmt.Errorf("用户总流量已用完，无法恢复服务")
	}
	tunnelFlow := ut.InFlow + ut.OutFlow
	if ut.Flow*forwardBytesToGB <= tunnelFlow {
		return fmt.Errorf("该隧道流量已用完，无法恢复服务")
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
