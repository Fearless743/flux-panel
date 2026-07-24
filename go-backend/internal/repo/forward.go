package repo

import (
	"database/sql"
	"fmt"
	"strings"

	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/jmoiron/sqlx"
)

type ForwardRepo struct {
	DB *sqlx.DB
}

func NewForwardRepo(db *sqlx.DB) *ForwardRepo {
	return &ForwardRepo{DB: db}
}

// ForwardWithTunnel 列表项（对齐 ForwardWithTunnelDto）
type ForwardWithTunnel struct {
	ID          int64   `db:"id" json:"id"`
	UserID      int     `db:"user_id" json:"userId"`
	Name        string  `db:"name" json:"name"`
	TunnelID    int     `db:"tunnel_id" json:"tunnelId"`
	RemoteAddr  string  `db:"remote_addr" json:"remoteAddr"`
	Status      int     `db:"status" json:"status"`
	CreatedTime int64   `db:"created_time" json:"createdTime"`
	UpdatedTime int64   `db:"updated_time" json:"updatedTime"`
	UserName    string  `db:"user_name" json:"userName"`
	InFlow      int64   `db:"in_flow" json:"inFlow"`
	OutFlow     int64   `db:"out_flow" json:"outFlow"`
	Strategy    string  `db:"strategy" json:"strategy"`
	Inx         int     `db:"inx" json:"inx"`
	TunnelName  *string `db:"tunnel_name" json:"tunnelName"`
	Type        *int    `db:"type" json:"type,omitempty"`
	// 运行时填充
	InIP   string `db:"-" json:"inIp"`
	InPort *int   `db:"-" json:"inPort"`
}

func (r *ForwardRepo) GetByID(id int64) (*model.Forward, error) {
	var f model.Forward
	err := r.DB.Get(&f, `SELECT id, user_id, user_name, name, tunnel_id, remote_addr, strategy,
		in_flow, out_flow, created_time, updated_time, status, inx FROM forward WHERE id = ?`, id)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &f, nil
}

func (r *ForwardRepo) ListAll() ([]ForwardWithTunnel, error) {
	var list []ForwardWithTunnel
	err := r.DB.Select(&list, `
		SELECT f.id, f.user_id, f.name, f.tunnel_id, f.remote_addr, f.status,
			f.created_time, f.updated_time, f.user_name, f.in_flow, f.strategy,
			f.out_flow, f.inx, t.name AS tunnel_name, t.type
		FROM forward f
		LEFT JOIN tunnel t ON f.tunnel_id = t.id
		ORDER BY f.created_time DESC`)
	return list, err
}

func (r *ForwardRepo) ListByUserID(userID int64) ([]ForwardWithTunnel, error) {
	var list []ForwardWithTunnel
	err := r.DB.Select(&list, `
		SELECT f.id, f.user_id, f.name, f.tunnel_id, f.remote_addr, f.status,
			f.created_time, f.updated_time, f.user_name, f.in_flow, f.strategy,
			f.out_flow, f.inx, t.name AS tunnel_name, t.type
		FROM forward f
		LEFT JOIN tunnel t ON f.tunnel_id = t.id
		WHERE f.user_id = ?
		ORDER BY f.created_time DESC`, userID)
	return list, err
}

func (r *ForwardRepo) Insert(f *model.Forward) (int64, error) {
	res, err := r.DB.Exec(`
		INSERT INTO forward (user_id, user_name, name, tunnel_id, remote_addr, strategy,
			in_flow, out_flow, created_time, updated_time, status, inx)
		VALUES (?, ?, ?, ?, ?, ?, 0, 0, ?, ?, ?, ?)`,
		f.UserID, f.UserName, f.Name, f.TunnelID, f.RemoteAddr, f.Strategy,
		f.CreatedTime, f.UpdatedTime, f.Status, f.Inx)
	if err != nil {
		return 0, err
	}
	return res.LastInsertId()
}

func (r *ForwardRepo) Update(f *model.Forward) error {
	_, err := r.DB.Exec(`
		UPDATE forward SET name=?, tunnel_id=?, remote_addr=?, strategy=?,
			status=?, updated_time=?, inx=? WHERE id=?`,
		f.Name, f.TunnelID, f.RemoteAddr, f.Strategy,
		f.Status, f.UpdatedTime, f.Inx, f.ID)
	return err
}

func (r *ForwardRepo) UpdateStatus(id int64, status int, updatedTime int64) error {
	_, err := r.DB.Exec(`UPDATE forward SET status=?, updated_time=? WHERE id=?`, status, updatedTime, id)
	return err
}

func (r *ForwardRepo) UpdateInx(id int64, inx int) error {
	_, err := r.DB.Exec(`UPDATE forward SET inx=? WHERE id=?`, inx, id)
	return err
}

func (r *ForwardRepo) Delete(id int64) error {
	_, err := r.DB.Exec(`DELETE FROM forward WHERE id=?`, id)
	return err
}

func (r *ForwardRepo) ListByTunnelID(tunnelID int64) ([]model.Forward, error) {
	var list []model.Forward
	err := r.DB.Select(&list, `SELECT id, user_id, user_name, name, tunnel_id, remote_addr, strategy,
		in_flow, out_flow, created_time, updated_time, status, inx FROM forward WHERE tunnel_id=?`, tunnelID)
	return list, err
}

func (r *ForwardRepo) ListByUserAndTunnel(userID, tunnelID int) ([]model.Forward, error) {
	var list []model.Forward
	err := r.DB.Select(&list, `SELECT id, user_id, user_name, name, tunnel_id, remote_addr, strategy,
		in_flow, out_flow, created_time, updated_time, status, inx FROM forward WHERE user_id=? AND tunnel_id=?`,
		userID, tunnelID)
	return list, err
}

func (r *ForwardRepo) DeleteByTunnelID(tunnelID int64) error {
	_, err := r.DB.Exec(`DELETE FROM forward WHERE tunnel_id=?`, tunnelID)
	return err
}

func (r *ForwardRepo) CountByUser(userID int) (int64, error) {
	var n int64
	err := r.DB.Get(&n, `SELECT COUNT(1) FROM forward WHERE user_id=?`, userID)
	return n, err
}

func (r *ForwardRepo) CountByUserTunnel(userID, tunnelID int, excludeForwardID *int64) (int64, error) {
	var n int64
	var err error
	if excludeForwardID != nil {
		err = r.DB.Get(&n, `SELECT COUNT(1) FROM forward WHERE user_id=? AND tunnel_id=? AND id<>?`,
			userID, tunnelID, *excludeForwardID)
	} else {
		err = r.DB.Get(&n, `SELECT COUNT(1) FROM forward WHERE user_id=? AND tunnel_id=?`,
			userID, tunnelID)
	}
	return n, err
}

func (r *ForwardRepo) CountByIDsAndUser(ids []int64, userID int64) (int64, error) {
	if len(ids) == 0 {
		return 0, nil
	}
	query, args, err := sqlx.In(`SELECT COUNT(1) FROM forward WHERE id IN (?) AND user_id=?`, ids, userID)
	if err != nil {
		return 0, err
	}
	query = r.DB.Rebind(query)
	var n int64
	err = r.DB.Get(&n, query, args...)
	return n, err
}

// ---- 关联实体查询 ----

func (r *ForwardRepo) GetTunnel(id int64) (*model.Tunnel, error) {
	var t model.Tunnel
	err := r.DB.Get(&t, `SELECT id, name, traffic_ratio, type, protocol, flow,
		created_time, updated_time, status, in_ip FROM tunnel WHERE id=?`, id)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &t, nil
}

func (r *ForwardRepo) GetNode(id int64) (*model.Node, error) {
	var n model.Node
	err := r.DB.Get(&n, `SELECT id, name, secret, server_ip, port, interface_name, version,
		http, tls, socks, created_time, updated_time, status, tcp_listen_addr, udp_listen_addr
		FROM node WHERE id=?`, id)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &n, nil
}

func (r *ForwardRepo) GetUser(id int64) (*model.User, error) {
	var u model.User
	err := r.DB.Get(&u, `SELECT id, user, pwd, role_id, exp_time, flow, in_flow, out_flow,
		flow_reset_time, num, created_time, updated_time, status FROM user WHERE id=?`, id)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &u, nil
}

func (r *ForwardRepo) GetUserTunnel(userID, tunnelID int) (*model.UserTunnel, error) {
	var ut model.UserTunnel
	err := r.DB.Get(&ut, `SELECT id, user_id, tunnel_id, speed_id, num, flow, in_flow, out_flow,
		flow_reset_time, exp_time, status FROM user_tunnel WHERE user_id=? AND tunnel_id=?`,
		userID, tunnelID)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &ut, nil
}

// ListChainTunnelsByTunnel 按 tunnel_id 查询；chainType 为空则全部，否则按 chain_type 过滤（"1"/"2"/"3"）
func (r *ForwardRepo) ListChainTunnelsByTunnel(tunnelID int64, chainType string) ([]model.ChainTunnel, error) {
	var list []model.ChainTunnel
	var err error
	if chainType == "" {
		err = r.DB.Select(&list, `SELECT id, tunnel_id, chain_type, node_id, port, strategy, inx, protocol
			FROM chain_tunnel WHERE tunnel_id=?`, tunnelID)
	} else {
		err = r.DB.Select(&list, `SELECT id, tunnel_id, chain_type, node_id, port, strategy, inx, protocol
			FROM chain_tunnel WHERE tunnel_id=? AND chain_type=?`, tunnelID, chainType)
	}
	return list, err
}

func (r *ForwardRepo) ListChainTunnelsByNode(nodeID int64) ([]model.ChainTunnel, error) {
	var list []model.ChainTunnel
	err := r.DB.Select(&list, `SELECT id, tunnel_id, chain_type, node_id, port, strategy, inx, protocol
		FROM chain_tunnel WHERE node_id=?`, nodeID)
	return list, err
}

// ParsePorts 解析节点端口配置："1000-2000,3000"
func ParsePorts(input string) ([]int, error) {
	set := make(map[int]struct{})
	if strings.TrimSpace(input) == "" {
		return nil, nil
	}
	parts := strings.Split(input, ",")
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}
		if strings.Contains(part, "-") {
			rangeParts := strings.SplitN(part, "-", 2)
			if len(rangeParts) != 2 {
				return nil, fmt.Errorf("invalid port range: %s", part)
			}
			var start, end int
			if _, err := fmt.Sscanf(strings.TrimSpace(rangeParts[0]), "%d", &start); err != nil {
				return nil, err
			}
			if _, err := fmt.Sscanf(strings.TrimSpace(rangeParts[1]), "%d", &end); err != nil {
				return nil, err
			}
			if start > end {
				start, end = end, start
			}
			for i := start; i <= end; i++ {
				set[i] = struct{}{}
			}
		} else {
			var p int
			if _, err := fmt.Sscanf(part, "%d", &p); err != nil {
				return nil, err
			}
			set[p] = struct{}{}
		}
	}
	out := make([]int, 0, len(set))
	// 排序
	min, max := 0, 0
	first := true
	for p := range set {
		if first {
			min, max = p, p
			first = false
		} else {
			if p < min {
				min = p
			}
			if p > max {
				max = p
			}
		}
	}
	for i := min; i <= max; i++ {
		if _, ok := set[i]; ok {
			out = append(out, i)
		}
	}
	return out, nil
}
