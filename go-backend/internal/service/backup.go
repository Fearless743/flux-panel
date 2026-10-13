package service

import (
	"errors"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/jmoiron/sqlx"
)

// BackupData 全量数据备份载荷，网页端导入导出使用。
// 包含全部业务表（含用户密码哈希，可完整迁移恢复）。
type BackupData struct {
	Version         string                  `json:"version"`
	ExportedAt      int64                   `json:"exportedAt"`
	Users           []model.User            `json:"users"`
	Nodes           []model.Node            `json:"nodes"`
	Tunnels         []model.Tunnel          `json:"tunnels"`
	ChainTunnels    []model.ChainTunnel     `json:"chainTunnels"`
	Forwards        []model.Forward         `json:"forwards"`
	ForwardPorts    []model.ForwardPort     `json:"forwardPorts"`
	UserTunnels     []model.UserTunnel      `json:"userTunnels"`
	SpeedLimits     []model.SpeedLimit      `json:"speedLimits"`
	ViteConfigs     []model.ViteConfig      `json:"viteConfigs"`
	StatisticsFlows []model.StatisticsFlow  `json:"statisticsFlows"`
}

type BackupService struct {
	db *sqlx.DB
}

func NewBackupService(db *sqlx.DB) *BackupService {
	return &BackupService{db: db}
}

func selectAll[T any](db *sqlx.DB, query string) ([]T, error) {
	var list []T
	if err := db.Select(&list, query); err != nil {
		return nil, err
	}
	if list == nil {
		list = []T{}
	}
	return list, nil
}

// Export 导出全部数据（只读，无锁表，单连接串行下安全）
func (s *BackupService) Export() (*BackupData, error) {
	users, err := selectAll[model.User](s.db, `SELECT * FROM user ORDER BY id`)
	if err != nil {
		return nil, err
	}
	nodes, err := selectAll[model.Node](s.db, `SELECT * FROM node ORDER BY id`)
	if err != nil {
		return nil, err
	}
	tunnels, err := selectAll[model.Tunnel](s.db, `SELECT * FROM tunnel ORDER BY id`)
	if err != nil {
		return nil, err
	}
	chainTunnels, err := selectAll[model.ChainTunnel](s.db, `SELECT * FROM chain_tunnel ORDER BY id`)
	if err != nil {
		return nil, err
	}
	forwards, err := selectAll[model.Forward](s.db, `SELECT * FROM forward ORDER BY id`)
	if err != nil {
		return nil, err
	}
	forwardPorts, err := selectAll[model.ForwardPort](s.db, `SELECT * FROM forward_port ORDER BY id`)
	if err != nil {
		return nil, err
	}
	userTunnels, err := selectAll[model.UserTunnel](s.db, `SELECT * FROM user_tunnel ORDER BY id`)
	if err != nil {
		return nil, err
	}
	speedLimits, err := selectAll[model.SpeedLimit](s.db, `SELECT * FROM speed_limit ORDER BY id`)
	if err != nil {
		return nil, err
	}
	viteConfigs, err := selectAll[model.ViteConfig](s.db, `SELECT * FROM vite_config ORDER BY id`)
	if err != nil {
		return nil, err
	}
	statisticsFlows, err := selectAll[model.StatisticsFlow](s.db, `SELECT * FROM statistics_flow ORDER BY id`)
	if err != nil {
		return nil, err
	}
	return &BackupData{
		Version:         "1",
		ExportedAt:      time.Now().UnixMilli(),
		Users:           users,
		Nodes:           nodes,
		Tunnels:         tunnels,
		ChainTunnels:    chainTunnels,
		Forwards:        forwards,
		ForwardPorts:    forwardPorts,
		UserTunnels:     userTunnels,
		SpeedLimits:     speedLimits,
		ViteConfigs:     viteConfigs,
		StatisticsFlows: statisticsFlows,
	}, nil
}

// Import 全量覆盖式恢复：先清空全部业务表，再按 ID 原样插入，保证外键关联（forward→user/tunnel、forward_port→forward/node）不变。
// 调用方必须已做管理员鉴权与前端二次确认。
func (s *BackupService) Import(data *BackupData) error {
	if data == nil {
		return errors.New("备份数据不能为空")
	}
	if data.Version != "" && data.Version != "1" && data.Version != "1.0" {
		return errors.New("不支持的备份版本")
	}
	// 必须包含至少一个管理员，防止恢复后无人可登录
	hasAdmin := false
	for _, u := range data.Users {
		if u.RoleID == 0 {
			hasAdmin = true
			break
		}
	}
	if !hasAdmin {
		return errors.New("备份文件中缺少管理员账号，为防止面板无法登录已拒绝导入")
	}

	tx, err := s.db.Beginx()
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()

	// 清空（子表先清，避免外键约束问题；SQLite 默认无 FK，但保持顺序）
	for _, stmt := range []string{
		`DELETE FROM forward_port`,
		`DELETE FROM forward`,
		`DELETE FROM user_tunnel`,
		`DELETE FROM statistics_flow`,
		`DELETE FROM chain_tunnel`,
		`DELETE FROM speed_limit`,
		`DELETE FROM tunnel`,
		`DELETE FROM node`,
		`DELETE FROM user`,
		`DELETE FROM vite_config`,
	} {
		if _, err := tx.Exec(stmt); err != nil {
			return err
		}
	}

	// 插入（显式 ID，保证关联关系不变；仅写 schema.sql 真实存在的列）
	for _, u := range data.Users {
		if _, err := tx.Exec(
			`INSERT INTO user (id, user, pwd, role_id, exp_time, flow, in_flow, out_flow, flow_reset_time, num, created_time, updated_time, status)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			u.ID, u.User, u.Pwd, u.RoleID, u.ExpTime, u.Flow, u.InFlow, u.OutFlow, u.FlowResetTime, u.Num, u.CreatedTime, u.UpdatedTime, u.Status,
		); err != nil {
			return err
		}
	}
	for _, n := range data.Nodes {
		if _, err := tx.Exec(
			`INSERT INTO node (id, name, secret, server_ip, port, interface_name, version, http, tls, socks, created_time, updated_time, status, tcp_listen_addr, udp_listen_addr)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			n.ID, n.Name, n.Secret, n.ServerIP, n.Port, n.InterfaceName, n.Version, n.HTTP, n.TLS, n.Socks, n.CreatedTime, n.UpdatedTime, n.Status, n.TCPListenAddr, n.UDPListenAddr,
		); err != nil {
			return err
		}
	}
	for _, t := range data.Tunnels {
		if _, err := tx.Exec(
			`INSERT INTO tunnel (id, name, traffic_ratio, type, protocol, flow, created_time, updated_time, status, in_ip)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			t.ID, t.Name, t.TrafficRatio, t.Type, t.Protocol, t.Flow, t.CreatedTime, t.UpdatedTime, t.Status, t.InIP,
		); err != nil {
			return err
		}
	}
	for _, ct := range data.ChainTunnels {
		if _, err := tx.Exec(
			`INSERT INTO chain_tunnel (id, tunnel_id, chain_type, node_id, port, strategy, inx, protocol, exit_node_ids)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			ct.ID, ct.TunnelID, ct.ChainType, ct.NodeID, ct.Port, ct.Strategy, ct.Inx, ct.Protocol, ct.ExitNodeIDs,
		); err != nil {
			return err
		}
	}
	for _, f := range data.Forwards {
		if _, err := tx.Exec(
			`INSERT INTO forward (id, user_id, user_name, name, tunnel_id, remote_addr, strategy, in_flow, out_flow, created_time, updated_time, status, inx)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			f.ID, f.UserID, f.UserName, f.Name, f.TunnelID, f.RemoteAddr, f.Strategy, f.InFlow, f.OutFlow, f.CreatedTime, f.UpdatedTime, f.Status, f.Inx,
		); err != nil {
			return err
		}
	}
	for _, fp := range data.ForwardPorts {
		if _, err := tx.Exec(
			`INSERT INTO forward_port (id, forward_id, node_id, port) VALUES (?, ?, ?, ?)`,
			fp.ID, fp.ForwardID, fp.NodeID, fp.Port,
		); err != nil {
			return err
		}
	}
	for _, ut := range data.UserTunnels {
		if _, err := tx.Exec(
			`INSERT INTO user_tunnel (id, user_id, tunnel_id, speed_id, num, flow, in_flow, out_flow, flow_reset_time, exp_time, status)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
			ut.ID, ut.UserID, ut.TunnelID, ut.SpeedID, ut.Num, ut.Flow, ut.InFlow, ut.OutFlow, ut.FlowResetTime, ut.ExpTime, ut.Status,
		); err != nil {
			return err
		}
	}
	for _, sl := range data.SpeedLimits {
		if _, err := tx.Exec(
			`INSERT INTO speed_limit (id, name, speed, tunnel_id, tunnel_name, created_time, updated_time, status)
			 VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
			sl.ID, sl.Name, sl.Speed, sl.TunnelID, sl.TunnelName, sl.CreatedTime, sl.UpdatedTime, sl.Status,
		); err != nil {
			return err
		}
	}
	for _, vc := range data.ViteConfigs {
		if _, err := tx.Exec(
			`INSERT INTO vite_config (id, name, value, time) VALUES (?, ?, ?, ?)`,
			vc.ID, vc.Name, vc.Value, vc.Time,
		); err != nil {
			return err
		}
	}
	for _, sf := range data.StatisticsFlows {
		if _, err := tx.Exec(
			`INSERT INTO statistics_flow (id, user_id, flow, total_flow, time, created_time) VALUES (?, ?, ?, ?, ?, ?)`,
			sf.ID, sf.UserID, sf.Flow, sf.TotalFlow, sf.Time, sf.CreatedTime,
		); err != nil {
			return err
		}
	}

	// 修正自增序列，避免后续插入 ID 冲突
	for _, tbl := range []string{"user", "node", "tunnel", "chain_tunnel", "forward", "forward_port", "user_tunnel", "speed_limit", "vite_config", "statistics_flow"} {
		_, _ = tx.Exec(`UPDATE sqlite_sequence SET seq = (SELECT COALESCE(MAX(id),0) FROM `+tbl+`) WHERE name = ?`, tbl)
	}

	return tx.Commit()
}
