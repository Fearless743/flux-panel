package service

import (
	"database/sql"
	"fmt"
	"log/slog"
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

		remainEntry := 0
		for _, ct := range all {
			if ct.NodeID == nodeID {
				continue
			}
			if chainTypeOf(ct) == 1 {
				remainEntry++
			}
		}
		if remainEntry == 0 {
			blockers = append(blockers, fmt.Sprintf("隧道[%s]剔除后将没有入口节点", t.Name))
		}
	}

	if len(blockers) > 0 {
		return fmt.Errorf("无法删除节点：%s。请先在隧道编辑中更换入口，或删除相关隧道后再删节点。",
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

		wasEntry := false
		for _, ct := range all {
			if ct.NodeID != nodeID {
				continue
			}
			if chainTypeOf(ct) == 1 {
				wasEntry = true
			}
		}

		slog.Info("删除节点时从隧道剔除",
			"nodeId", nodeID, "tunnelId", tunnelID, "tunnelName", tunnel.Name,
			"entry", wasEntry)

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
		for _, ct := range remaining {
			if chainTypeOf(ct) == 1 {
				entries = append(entries, ct)
			}
		}

		// 4) 刷新 in_ip
		refreshTunnelInIP(db, tunnel, entries)
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


