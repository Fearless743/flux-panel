package service

import (
	"encoding/json"
	"fmt"
	"math/rand"
	"strings"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/gost"
	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/repo"
	"github.com/Fearless743/flux-panel/go-backend/internal/ws"
	"github.com/jmoiron/sqlx"
)

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
	Name       string  `json:"name"`
	TunnelID   int     `json:"tunnelId"`
	RemoteAddr string  `json:"remoteAddr"`
	Strategy   string  `json:"strategy"`
	InPort     *int    `json:"inPort"`
}

type ForwardUpdateReq struct {
	ID         int64   `json:"id"`
	UserID     int     `json:"userId"`
	Name       string  `json:"name"`
	TunnelID   *int    `json:"tunnelId"`
	RemoteAddr string  `json:"remoteAddr"`
	Strategy   string  `json:"strategy"`
	InPort     *int    `json:"inPort"`
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
	NodeID        int64    `json:"nodeId"`
	NodeName      string   `json:"nodeName"`
	TargetIP      string   `json:"targetIp"`
	TargetPort    int      `json:"targetPort"`
	Description   string   `json:"description"`
	Success       bool     `json:"success"`
	Message       string   `json:"message"`
	AverageTime   float64  `json:"averageTime"`
	PacketLoss    float64  `json:"packetLoss"`
	Timestamp     int64    `json:"timestamp"`
	FromChainType *int     `json:"fromChainType,omitempty"`
	FromInx       *int     `json:"fromInx,omitempty"`
	ToChainType   *int     `json:"toChainType,omitempty"`
	ToInx         *int     `json:"toInx,omitempty"`
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

	for i := range list {
		s.fillInIP(&list[i])
	}
	return list, nil
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

	inPort := req.InPort
	if inPort == nil {
		oldPorts, _ := s.Port.ListByForwardID(exist.ID)
		if len(oldPorts) > 0 {
			oldPort := oldPorts[0].Port
			availableOnAll := true
			for _, ct := range newEntryNodes {
				avail, err := s.getNodePort(ct.NodeID, exist.ID)
				if err != nil {
					return err
				}
				if !containsInt(avail, oldPort) {
					availableOnAll = false
					break
				}
			}
			if availableOnAll {
				inPort = &oldPort
			}
		}
	}
	newEntryNodes, err = s.getPort(newEntryNodes, inPort, exist.ID)
	if err != nil {
		return err
	}

	// 删旧服务
	oldUserTunnel, _ := s.Repo.GetUserTunnel(exist.UserID, int(oldTunnel.ID))
	oldServiceName := s.buildServiceName(exist.ID, int64(exist.UserID), oldUserTunnel)
	oldEntryNodes, _ := s.Repo.ListChainTunnelsByTunnel(oldTunnel.ID, "1")
	for _, ct := range oldEntryNodes {
		names := []string{oldServiceName + "_tcp", oldServiceName + "_udp"}
		res := s.Hub.SendMsg(ct.NodeID, gost.DeleteServicePayload(names), "DeleteService")
		_ = gost.NormalizeOK(res.Msg)
	}
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

	// 新服务
	newServiceName := s.buildServiceName(exist.ID, int64(exist.UserID), newUserTunnel)
	var limiter *int
	if newUserTunnel != nil {
		limiter = newUserTunnel.SpeedID
	}
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
		msg := s.addOrUpdateService(newServiceName, limiter, node, exist, fp, newTunnel, "AddService")
		if !gost.IsOK(msg) {
			return fmt.Errorf("节点[%s]服务创建失败: %s", node.Name, gost.NormalizeOK(msg))
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
	for _, ct := range chainTunnels {
		node, err := s.Repo.GetNode(ct.NodeID)
		if err != nil {
			return err
		}
		if node == nil {
			return fmt.Errorf("部分节点不存在")
		}
		names := []string{serviceName + "_tcp", serviceName + "_udp"}
		res := s.Hub.SendMsg(node.ID, gost.DeleteServicePayload(names), "DeleteService")
		_ = gost.NormalizeOK(res.Msg)
	}
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
	for _, ct := range chainTunnels {
		node, err := s.Repo.GetNode(ct.NodeID)
		if err != nil {
			return err
		}
		if node == nil {
			return fmt.Errorf("部分节点不存在")
		}
		res := s.Hub.SendMsg(node.ID, gost.PauseResumePayload(serviceName), gostMethod)
		if !gost.IsOK(res.Msg) {
			return fmt.Errorf("%s", gost.NormalizeOK(res.Msg))
		}
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

func (s *ForwardService) BatchDelete(user CurrentUser, ids []int64) (string, bool) {
	if len(ids) == 0 {
		return "请选择要删除的转发", false
	}
	var errors []string
	success := 0
	for _, id := range ids {
		if err := s.Delete(user, id); err != nil {
			errors = append(errors, fmt.Sprintf("ID-%d: %s", id, err.Error()))
		} else {
			success++
		}
	}
	msg := fmt.Sprintf("成功删除%d个转发", success)
	if len(errors) > 0 {
		msg += "，失败" + fmt.Sprintf("%d", len(errors)) + "个: " + strings.Join(errors, "; ")
	}
	return msg, success > 0
}

func (s *ForwardService) BatchPause(user CurrentUser, ids []int64) (string, bool) {
	if len(ids) == 0 {
		return "请选择要暂停的转发", false
	}
	var errors []string
	success := 0
	for _, id := range ids {
		if err := s.Pause(user, id); err != nil {
			errors = append(errors, fmt.Sprintf("ID-%d: %s", id, err.Error()))
		} else {
			success++
		}
	}
	msg := fmt.Sprintf("成功暂停%d个转发", success)
	if len(errors) > 0 {
		msg += "，失败" + fmt.Sprintf("%d", len(errors)) + "个: " + strings.Join(errors, "; ")
	}
	return msg, success > 0
}

func (s *ForwardService) BatchResume(user CurrentUser, ids []int64) (string, bool) {
	if len(ids) == 0 {
		return "请选择要恢复的转发", false
	}
	var errors []string
	success := 0
	for _, id := range ids {
		if err := s.Resume(user, id); err != nil {
			errors = append(errors, fmt.Sprintf("ID-%d: %s", id, err.Error()))
		} else {
			success++
		}
	}
	msg := fmt.Sprintf("成功恢复%d个转发", success)
	if len(errors) > 0 {
		msg += "，失败" + fmt.Sprintf("%d", len(errors)) + "个: " + strings.Join(errors, "; ")
	}
	return msg, success > 0
}

func (s *ForwardService) BatchChangeTunnel(user CurrentUser, ids []int64, tunnelID int) (string, bool) {
	if len(ids) == 0 {
		return "请选择要更改隧道的转发", false
	}
	if tunnelID == 0 {
		return "请选择目标隧道", false
	}
	var errors []string
	success := 0
	for _, id := range ids {
		exist, err := s.validateForwardExists(id, user)
		if err != nil || exist == nil {
			errors = append(errors, fmt.Sprintf("ID-%d: 转发不存在或无权限", id))
			continue
		}
		tid := tunnelID
		req := ForwardUpdateReq{
			ID:         id,
			TunnelID:   &tid,
			Name:       exist.Name,
			UserID:     exist.UserID,
			RemoteAddr: exist.RemoteAddr,
			Strategy:   exist.Strategy,
		}
		if err := s.Update(user, req); err != nil {
			errors = append(errors, fmt.Sprintf("ID-%d: %s", id, err.Error()))
		} else {
			success++
		}
	}
	msg := fmt.Sprintf("成功迁移%d个转发", success)
	if len(errors) > 0 {
		msg += "，失败" + fmt.Sprintf("%d", len(errors)) + "个: " + strings.Join(errors, "; ")
	}
	return msg, success > 0
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
