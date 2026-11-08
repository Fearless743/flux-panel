package service

import (
	"fmt"
	"log/slog"
	"strconv"
	"strings"
	"sync"

	"github.com/Fearless743/flux-panel/go-backend/internal/gost"
	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/repo"
	"github.com/Fearless743/flux-panel/go-backend/internal/ws"
	"github.com/jmoiron/sqlx"
)

// NodeSyncService 节点上线后按 DB 重放期望配置（limiter → chain → service → pause）
type NodeSyncService struct {
	DB  *sqlx.DB
	Hub *ws.Hub

	Node     *repo.NodeRepo
	Tunnel   *repo.TunnelRepo
	Chain    *repo.ChainTunnelRepo
	Forward  *repo.ForwardRepo
	Port     *repo.ForwardPortRepo
	UserTunn *repo.UserTunnelRepo
	Speed    *repo.SpeedLimitRepo
}

func NewNodeSyncService(db *sqlx.DB, hub *ws.Hub) *NodeSyncService {
	return &NodeSyncService{
		DB:       db,
		Hub:      hub,
		Node:     repo.NewNodeRepo(db),
		Tunnel:   repo.NewTunnelRepo(db),
		Chain:    repo.NewChainTunnelRepo(db),
		Forward:  repo.NewForwardRepo(db),
		Port:     repo.NewForwardPortRepo(db),
		UserTunn: repo.NewUserTunnelRepo(db),
		Speed:    repo.NewSpeedLimitRepo(db),
	}
}

var nodeSyncLocks sync.Map // map[int64]*sync.Mutex

func lockNodeSync(nodeID int64) func() {
	v, _ := nodeSyncLocks.LoadOrStore(nodeID, &sync.Mutex{})
	mu := v.(*sync.Mutex)
	mu.Lock()
	return mu.Unlock
}

// SyncNodeConfig 将 DB 中该节点应有配置通过现有增量命令重放到节点。
// 设计为上线后异步调用；单条失败记日志并继续。
func (s *NodeSyncService) SyncNodeConfig(nodeID int64) {
	if s == nil || s.Hub == nil || nodeID == 0 {
		return
	}
	unlock := lockNodeSync(nodeID)
	defer unlock()

	if !s.Hub.IsNodeOnline(nodeID) {
		slog.Info("节点配置同步跳过：不在线", "nodeId", nodeID)
		return
	}

	node, err := s.Node.GetByID(nodeID)
	if err != nil || node == nil {
		slog.Warn("节点配置同步：加载节点失败", "nodeId", nodeID, "err", err)
		return
	}

	links, err := s.Chain.ListByNodeID(nodeID)
	if err != nil {
		slog.Warn("节点配置同步：查询 chain_tunnel 失败", "nodeId", nodeID, "err", err)
		return
	}

	tunnelIDs := make([]int64, 0)
	tunnelIDSet := make(map[int64]struct{})
	for _, ct := range links {
		if _, ok := tunnelIDSet[ct.TunnelID]; !ok {
			tunnelIDSet[ct.TunnelID] = struct{}{}
			tunnelIDs = append(tunnelIDs, ct.TunnelID)
		}
	}

	var okN, failN int
	var recMu sync.Mutex
	record := func(label string, msg string) {
		recMu.Lock()
		defer recMu.Unlock()
		if gost.IsOK(msg) {
			okN++
			return
		}
		failN++
		slog.Warn("节点配置同步项失败", "nodeId", nodeID, "item", label, "msg", msg)
	}

	// 1) Limiters：多限速器并发下发（IO 等待重叠，同节点写由 hub per-conn 锁串行化）
	if len(tunnelIDs) > 0 {
		limiters, err := s.listSpeedLimitsByTunnelIDs(tunnelIDs)
		if err != nil {
			slog.Warn("节点配置同步：查询限速失败", "nodeId", nodeID, "err", err)
		} else {
			parallelEach(len(limiters), 8, func(i int) string {
				sl := &limiters[i]
				record(fmt.Sprintf("limiter:%d", sl.ID), s.ensureLimiter(nodeID, sl))
				return ""
			})
		}
	}

	// Forwards on this node (入口端口)：per-forward 并发重放
	ports, err := s.Port.ListByNodeID(nodeID)
	if err != nil {
		slog.Warn("节点配置同步：查询 forward_port 失败", "nodeId", nodeID, "err", err)
	} else {
		fwSvc := NewForwardService(s.DB, s.Hub)
		parallelEach(len(ports), 8, func(i int) string {
			fp := &ports[i]
			fw, err := s.Forward.GetByID(fp.ForwardID)
			if err != nil || fw == nil {
				return ""
			}
			tunnel, err := s.Tunnel.GetByID(int64(fw.TunnelID))
			if err != nil || tunnel == nil {
				return ""
			}
			var ut *model.UserTunnel
			var limiter *int
			if fw.UserID != 0 {
				ut, _ = s.UserTunn.GetByUserAndTunnel(fw.UserID, fw.TunnelID)
				if ut != nil {
					limiter = ut.SpeedID
				}
			}
			base := fwSvc.buildServiceName(fw.ID, int64(fw.UserID), ut)
			// 空节点无本地 gost.json：Update 不存在时回落 Add
			msg := fwSvc.addOrUpdateService(base, limiter, node, fw, fp, tunnel, "UpdateService")
			if !gost.IsOK(msg) {
				msg = fwSvc.addOrUpdateService(base, limiter, node, fw, fp, tunnel, "AddService")
			}
			// exists 也算成功
			recMu.Lock()
			if gost.IsOK(msg) {
				okN++
			} else {
				failN++
				slog.Warn("节点配置同步项失败", "nodeId", nodeID, "item", "forward:"+base, "msg", msg)
			}
			recMu.Unlock()
			if fw.Status == 0 {
				res := s.Hub.SendMsg(nodeID, gost.PauseResumePayload(base), "PauseService")
				record("pause:"+base, gost.NormalizeOK(res.Msg))
			}
			return ""
		})
	}

	slog.Info("节点配置同步完成", "nodeId", nodeID, "ok", okN, "fail", failN)
}


func (s *NodeSyncService) ensureLimiter(nodeID int64, sl *model.SpeedLimit) string {
	speed := convertBitsToMBps(sl.Speed)
	data := gost.LimiterData(sl.ID, speed)
	payload := map[string]any{
		"limiter": strconv.FormatInt(sl.ID, 10),
		"data":    data,
	}
	res := s.Hub.SendMsg(nodeID, payload, "UpdateLimiters")
	if gost.IsOK(res.Msg) {
		return gost.NormalizeOK(res.Msg)
	}
	if strings.Contains(res.Msg, "not found") {
		res = s.Hub.SendMsg(nodeID, data, "AddLimiters")
		return gost.NormalizeOK(res.Msg)
	}
	// Update 失败时尝试 Add（空节点常见）
	res2 := s.Hub.SendMsg(nodeID, data, "AddLimiters")
	if gost.IsOK(res2.Msg) {
		return gost.NormalizeOK(res2.Msg)
	}
	return res.Msg
}

func (s *NodeSyncService) listSpeedLimitsByTunnelIDs(tunnelIDs []int64) ([]model.SpeedLimit, error) {
	if len(tunnelIDs) == 0 {
		return nil, nil
	}
	q, args, err := sqlx.In(
		`SELECT id, name, speed, tunnel_id, tunnel_name, created_time, updated_time, status
		 FROM speed_limit WHERE tunnel_id IN (?)`, tunnelIDs,
	)
	if err != nil {
		return nil, err
	}
	q = s.DB.Rebind(q)
	var list []model.SpeedLimit
	if err := s.DB.Select(&list, q, args...); err != nil {
		return nil, err
	}
	return list, nil
}

