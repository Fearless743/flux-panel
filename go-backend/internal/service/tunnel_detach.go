package service

import (
	"database/sql"
	"fmt"
	"log/slog"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/gost"
	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/ws"
	"github.com/jmoiron/sqlx"
)

// DetachNodeFromTunnels 从所有隧道拓扑中剔除节点，不删除隧道本身。
// 对齐 Java TunnelServiceImpl.detachNodeFromTunnels。
// 失败返回 error（中文文案）；成功可能仅记警告日志。
func DetachNodeFromTunnels(db *sqlx.DB, hub *ws.Hub, nodeID int64) error {
	if nodeID == 0 {
		return fmt.Errorf("节点ID不能为空")
	}

	var nodeLinks []model.ChainTunnel
	if err := db.Select(&nodeLinks, `SELECT * FROM chain_tunnel WHERE node_id = ?`, nodeID); err != nil {
		return err
	}
	if len(nodeLinks) == 0 {
		_, _ = db.Exec(`DELETE FROM forward_port WHERE node_id = ?`, nodeID)
		return nil
	}

	tunnelIDSet := make(map[int64]struct{})
	var tunnelIDs []int64
	for _, ct := range nodeLinks {
		if _, ok := tunnelIDSet[ct.TunnelID]; !ok {
			tunnelIDSet[ct.TunnelID] = struct{}{}
			tunnelIDs = append(tunnelIDs, ct.TunnelID)
		}
	}

	type tunnelCtx struct {
		tunnel *model.Tunnel
		all    []model.ChainTunnel
	}
	ctxMap := make(map[int64]*tunnelCtx)
	var blockers []string

	for _, tunnelID := range tunnelIDs {
		var t model.Tunnel
		err := db.Get(&t, `SELECT * FROM tunnel WHERE id = ?`, tunnelID)
		if err == sql.ErrNoRows {
			continue
		}
		if err != nil {
			return err
		}
		var all []model.ChainTunnel
		if err := db.Select(&all, `SELECT * FROM chain_tunnel WHERE tunnel_id = ?`, tunnelID); err != nil {
			return err
		}
		ctxMap[tunnelID] = &tunnelCtx{tunnel: &t, all: all}

		remainEntry, remainExit := 0, 0
		for _, ct := range all {
			if ct.NodeID == nodeID {
				continue
			}
			switch chainTypeOf(ct) {
			case 1:
				remainEntry++
			case 3:
				remainExit++
			}
		}
		if remainEntry == 0 {
			blockers = append(blockers, fmt.Sprintf("隧道[%s]剔除后将没有入口节点", t.Name))
		}
		if t.Type == 2 && remainExit == 0 {
			blockers = append(blockers, fmt.Sprintf("隧道[%s]剔除后将没有出口节点", t.Name))
		}
	}

	if len(blockers) > 0 {
		return fmt.Errorf("无法删除节点：%s。请先在隧道编辑中更换入口/出口，或删除相关隧道后再删节点。",
			strings.Join(blockers, "；"))
	}

	var warnings []string

	for _, tunnelID := range tunnelIDs {
		ctx := ctxMap[tunnelID]
		if ctx == nil || ctx.tunnel == nil {
			continue
		}
		tunnel := ctx.tunnel
		all := ctx.all

		wasEntry, wasChain, wasExit := false, false, false
		for _, ct := range all {
			if ct.NodeID != nodeID {
				continue
			}
			switch chainTypeOf(ct) {
			case 1:
				wasEntry = true
			case 2:
				wasChain = true
			case 3:
				wasExit = true
			}
		}

		slog.Info("删除节点时从隧道剔除",
			"nodeId", nodeID, "tunnelId", tunnelID, "tunnelName", tunnel.Name,
			"entry", wasEntry, "chain", wasChain, "exit", wasExit)

		// 1) 清理该节点上的 gost 配置
		func() {
			defer func() {
				if r := recover(); r != nil {
					msg := fmt.Sprintf("%v", r)
					slog.Warn("清理 gost 配置异常", "err", msg)
					warnings = append(warnings, fmt.Sprintf("隧道[%s]节点端配置清理失败: %s", tunnel.Name, msg))
				}
			}()
			if wasEntry {
				var forwards []model.Forward
				_ = db.Select(&forwards, `SELECT * FROM forward WHERE tunnel_id = ?`, tunnelID)
				for _, fw := range forwards {
					utID := int64(0)
					var ut model.UserTunnel
					err := db.Get(&ut, `SELECT * FROM user_tunnel WHERE user_id = ? AND tunnel_id = ? LIMIT 1`,
						fw.UserID, tunnelID)
					if err == nil {
						utID = int64(ut.ID)
					}
					base := gost.ForwardServiceBase(fw.ID, int64(fw.UserID), utID)
					deleteService(hub, nodeID, []string{base + "_tcp", base + "_udp"})
				}
				if tunnel.Type == 2 {
					deleteChains(hub, nodeID, gost.ChainName(tunnelID))
				}
			}
			if wasChain {
				deleteChains(hub, nodeID, gost.ChainName(tunnelID))
				deleteService(hub, nodeID, []string{gost.TunnelTLS(tunnelID)})
			}
			if wasExit {
				deleteService(hub, nodeID, []string{gost.TunnelTLS(tunnelID)})
			}
		}()

		// 2) 删 DB 引用
		if _, err := db.Exec(`DELETE FROM chain_tunnel WHERE tunnel_id = ? AND node_id = ?`, tunnelID, nodeID); err != nil {
			return err
		}
		if wasEntry {
			var forwardIDs []int64
			_ = db.Select(&forwardIDs, `SELECT id FROM forward WHERE tunnel_id = ?`, tunnelID)
			if len(forwardIDs) > 0 {
				q, args, err := sqlx.In(`DELETE FROM forward_port WHERE node_id = ? AND forward_id IN (?)`, nodeID, forwardIDs)
				if err == nil {
					q = db.Rebind(q)
					_, _ = db.Exec(q, args...)
				}
			}
		}

		// 3) 剩余拓扑
		var remaining []model.ChainTunnel
		if err := db.Select(&remaining, `SELECT * FROM chain_tunnel WHERE tunnel_id = ?`, tunnelID); err != nil {
			return err
		}
		var entries []model.ChainTunnel
		var outs []model.ChainTunnel
		chainGroups := make(map[int][]model.ChainTunnel)
		for _, ct := range remaining {
			switch chainTypeOf(ct) {
			case 1:
				entries = append(entries, ct)
			case 2:
				inx := 0
				if ct.Inx != nil {
					inx = *ct.Inx
				}
				chainGroups[inx] = append(chainGroups[inx], ct)
			case 3:
				outs = append(outs, ct)
			}
		}
		var chainKeys []int
		for k := range chainGroups {
			chainKeys = append(chainKeys, k)
		}
		sort.Ints(chainKeys)
		var chains [][]model.ChainTunnel
		for _, k := range chainKeys {
			chains = append(chains, chainGroups[k])
		}

		// 4) 刷新 in_ip
		refreshTunnelInIP(db, tunnel, entries)

		// 5) type=2 重推（按节点分组并行，同节点 chain 先于 service）
		if tunnel.Type == 2 {
			nodeIDs := make(map[int64]struct{})
			for _, ct := range remaining {
				nodeIDs[ct.NodeID] = struct{}{}
			}
			nodes := loadNodesMap(db, nodeIDs)

			var jobs []nodeJob
			for _, entry := range entries {
				n := nodes[entry.NodeID]
				if n == nil || n.Status != 1 {
					warnings = append(warnings, fmt.Sprintf("隧道[%s]入口节点离线或缺失，跳过链重推", tunnel.Name))
					continue
				}
				var target []model.ChainTunnel
				if len(chains) == 0 {
					target = outs
				} else {
					target = chains[0]
				}
				if len(target) == 0 {
					warnings = append(warnings, fmt.Sprintf("隧道[%s]入口后无下一跳，跳过链重推", tunnel.Name))
					continue
				}
				e, t := entry, target
				jobs = append(jobs, nodeJob{nodeID: e.NodeID, run: func() string {
					if res := pushChains(hub, e.NodeID, t, nodes); !gost.IsOK(res.Msg) {
						return fmt.Sprintf("隧道[%s]入口链重推失败: %s", tunnel.Name, res.Msg)
					}
					return ""
				}})
			}

			for i := range chains {
				var target []model.ChainTunnel
				if i+1 < len(chains) {
					target = chains[i+1]
				} else {
					target = outs
				}
				for _, ct := range chains[i] {
					n := nodes[ct.NodeID]
					if n == nil || n.Status != 1 {
						warnings = append(warnings, fmt.Sprintf("隧道[%s]转发链节点离线或缺失，跳过重推", tunnel.Name))
						continue
					}
					c, t := ct, target
					jobs = append(jobs, nodeJob{nodeID: c.NodeID, run: func() string {
						var msgs []string
						if len(t) > 0 {
							if res := pushChains(hub, c.NodeID, t, nodes); !gost.IsOK(res.Msg) {
								msgs = append(msgs, fmt.Sprintf("隧道[%s]转发链重推失败: %s", tunnel.Name, res.Msg))
							}
						}
						if res := pushChainService(hub, c.NodeID, c, nodes); !gost.IsOK(res.Msg) {
							msgs = append(msgs, fmt.Sprintf("隧道[%s]转发链服务重推失败: %s", tunnel.Name, res.Msg))
						}
						return strings.Join(msgs, "；")
					}})
				}
			}

			for _, ct := range outs {
				n := nodes[ct.NodeID]
				if n == nil || n.Status != 1 {
					warnings = append(warnings, fmt.Sprintf("隧道[%s]出口节点离线或缺失，跳过服务重推", tunnel.Name))
					continue
				}
				c := ct
				jobs = append(jobs, nodeJob{nodeID: c.NodeID, run: func() string {
					if res := pushChainService(hub, c.NodeID, c, nodes); !gost.IsOK(res.Msg) {
						return fmt.Sprintf("隧道[%s]出口服务重推失败: %s", tunnel.Name, res.Msg)
					}
					return ""
				}})
			}
			warnings = append(warnings, runNodeJobs(jobs)...)
		}
	}

	// 兜底清理 forward_port
	_, _ = db.Exec(`DELETE FROM forward_port WHERE node_id = ?`, nodeID)

	if len(warnings) > 0 {
		slog.Warn("删除节点时部分隧道重推/清理有警告", "nodeId", nodeID, "warnings", warnings)
		// DB 已剔除成功，不阻断删除
	}
	return nil
}

func chainTypeOf(ct model.ChainTunnel) int {
	n, _ := strconv.Atoi(strings.TrimSpace(ct.ChainType))
	return n
}

func deleteService(hub *ws.Hub, nodeID int64, names []string) {
	if hub == nil {
		return
	}
	// 清理类删除无需等待节点确认，避免慢响应时被 10s 超时拖住
	hub.SendNoWait(nodeID, gost.DeleteServicePayload(names), "DeleteService")
}

func deleteChains(hub *ws.Hub, nodeID int64, name string) {
	if hub == nil {
		return
	}
	hub.SendNoWait(nodeID, map[string]any{"chain": name}, "DeleteChains")
}

func loadNodesMap(db *sqlx.DB, ids map[int64]struct{}) map[int64]*model.Node {
	out := make(map[int64]*model.Node)
	if len(ids) == 0 {
		return out
	}
	idList := make([]int64, 0, len(ids))
	for id := range ids {
		idList = append(idList, id)
	}
	q, args, err := sqlx.In(`SELECT * FROM node WHERE id IN (?)`, idList)
	if err != nil {
		return out
	}
	q = db.Rebind(q)
	var list []model.Node
	if err := db.Select(&list, q, args...); err != nil {
		return out
	}
	for i := range list {
		n := list[i]
		out[n.ID] = &n
	}
	return out
}

func refreshTunnelInIP(db *sqlx.DB, tunnel *model.Tunnel, entries []model.ChainTunnel) {
	if tunnel == nil {
		return
	}
	var parts []string
	for _, entry := range entries {
		var n model.Node
		err := db.Get(&n, `SELECT * FROM node WHERE id = ?`, entry.NodeID)
		if err != nil || n.ServerIP == "" {
			continue
		}
		parts = append(parts, n.ServerIP)
	}
	inIP := strings.Join(parts, ",")
	now := time.Now().UnixMilli()
	_, _ = db.Exec(`UPDATE tunnel SET in_ip = ?, updated_time = ? WHERE id = ?`, inIP, now, tunnel.ID)
}

func pushChains(hub *ws.Hub, nodeID int64, target []model.ChainTunnel, nodes map[int64]*model.Node) ws.GostResult {
	if hub == nil || len(target) == 0 {
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
	// Update 优先
	req := map[string]any{
		"chain": data["name"],
		"data":  data,
	}
	res := hub.SendMsg(nodeID, req, "UpdateChains")
	// 空节点 Update 返回 not found，必须 Add（IsOK 不再吞掉 not found）
	if !gost.IsOK(res.Msg) {
		res = hub.SendMsg(nodeID, data, "AddChains")
	}
	res.Msg = gost.NormalizeOK(res.Msg)
	return res
}

func pushChainService(hub *ws.Hub, nodeID int64, ct model.ChainTunnel, nodes map[int64]*model.Node) ws.GostResult {
	if hub == nil {
		return ws.GostResult{Msg: "节点不在线"}
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
	services := gost.BuildChainService(ct.TunnelID, chainTypeOf(ct), proto, tcpListen, port, iface, srcHas)
	res := hub.SendMsg(nodeID, services, "UpdateService")
	if !gost.IsOK(res.Msg) {
		res = hub.SendMsg(nodeID, services, "AddService")
	}
	res.Msg = gost.NormalizeOK(res.Msg)
	return res
}
