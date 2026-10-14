package repo

import (
	"database/sql"
	"fmt"
	"sort"
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

// ForwardListFilter 分页列表的服务端过滤条件（高性能分页：WHERE 下推 + LIMIT/OFFSET）。
// UserID 为 nil 表示管理员（不过滤归属）；Keyword 匹配 name/remote_addr/user_name。
type ForwardListFilter struct {
	UserID   *int64
	TunnelID *int
	Status   *int
	Keyword  string
}

// buildFilteredWhere 组装 WHERE 子句与参数（Count 与 List 共用，保证总数与分页一致）。
// Keyword 中的 LIKE 通配符（% _ \）做转义，查询统一加 ESCAPE '\'。
func (f ForwardListFilter) buildFilteredWhere() (string, []any) {
	var sb strings.Builder
	args := make([]any, 0, 8)
	sb.WriteString("1=1")
	if f.UserID != nil {
		sb.WriteString(" AND f.user_id = ?")
		args = append(args, *f.UserID)
	}
	if f.TunnelID != nil {
		sb.WriteString(" AND f.tunnel_id = ?")
		args = append(args, *f.TunnelID)
	}
	if f.Status != nil {
		sb.WriteString(" AND f.status = ?")
		args = append(args, *f.Status)
	}
	if kw := strings.TrimSpace(f.Keyword); kw != "" {
		esc := strings.NewReplacer(`\`, `\\`, `%`, `\%`, `_`, `\_`).Replace(kw)
		like := "%" + esc + "%"
		sb.WriteString(" AND (f.name LIKE ? ESCAPE '\\' OR f.remote_addr LIKE ? ESCAPE '\\' OR f.user_name LIKE ? ESCAPE '\\')")
		args = append(args, like, like, like)
	}
	return sb.String(), args
}

// CountFiltered 分页总数（与 ListFiltered 同一 WHERE，保证 total 准确）。
func (r *ForwardRepo) CountFiltered(f ForwardListFilter) (int64, error) {
	where, args := f.buildFilteredWhere()
	var n int64
	err := r.DB.Get(&n, `SELECT COUNT(1) FROM forward f WHERE `+where, args...)
	return n, err
}

// ListFiltered 分页取一页（只查一页行 + JOIN 隧道名，inIp/inPort 由上层 FillInIPBatch 按页填充）。
// 排序固定 inx ASC, created_time DESC, id DESC：与前端拖拽排序语义一致，
// 全 0 inx 时退化为 created_time DESC（兼容历史顺序），且排序列均有索引覆盖。
func (r *ForwardRepo) ListFiltered(f ForwardListFilter, limit, offset int) ([]ForwardWithTunnel, error) {
	where, args := f.buildFilteredWhere()
	args = append(args, limit, offset)
	var list []ForwardWithTunnel
	err := r.DB.Select(&list, `
		SELECT f.id, f.user_id, f.name, f.tunnel_id, f.remote_addr, f.status,
			f.created_time, f.updated_time, f.user_name, f.in_flow, f.strategy,
			f.out_flow, f.inx, t.name AS tunnel_name, t.type
		FROM forward f
		LEFT JOIN tunnel t ON f.tunnel_id = t.id
		WHERE `+where+`
		ORDER BY f.inx ASC, f.created_time DESC, f.id DESC
		LIMIT ? OFFSET ?`, args...)
	if list == nil {
		list = []ForwardWithTunnel{}
	}
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

// chunkInt64IDs 按 size 切分 ID 列表（避开 SQLite 变量数上限 999）
func chunkInt64IDs(ids []int64, size int) [][]int64 {
	if size <= 0 {
		size = 500
	}
	out := make([][]int64, 0, (len(ids)+size-1)/size)
	for i := 0; i < len(ids); i += size {
		end := i + size
		if end > len(ids) {
			end = len(ids)
		}
		out = append(out, ids[i:end])
	}
	return out
}

// ListByIDs 批量按 ID 查询转发（批量操作预取，避免 N+1）
func (r *ForwardRepo) ListByIDs(ids []int64) ([]model.Forward, error) {	if len(ids) == 0 {
		return nil, nil
	}
	var out []model.Forward
	for _, chunk := range chunkInt64IDs(ids, 500) {
		q, args, err := sqlx.In(`SELECT id, user_id, user_name, name, tunnel_id, remote_addr, strategy,
			in_flow, out_flow, created_time, updated_time, status, inx FROM forward WHERE id IN (?)`, chunk)
		if err != nil {
			return nil, err
		}
		q = r.DB.Rebind(q)
		var list []model.Forward
		if err := r.DB.Select(&list, q, args...); err != nil {
			return nil, err
		}
		out = append(out, list...)
	}
	return out, nil
}

// ListTunnelsByIDs 批量查询隧道（用于 N+1 优化）
func (r *ForwardRepo) ListTunnelsByIDs(ids []int64) ([]model.Tunnel, error) {
	if len(ids) == 0 {
		return nil, nil
	}
	q, args, err := sqlx.In(`SELECT id, name, traffic_ratio, type, protocol, flow,
		created_time, updated_time, status, in_ip FROM tunnel WHERE id IN (?)`, ids)
	if err != nil {
		return nil, err
	}
	q = r.DB.Rebind(q)
	var list []model.Tunnel
	if err := r.DB.Select(&list, q, args...); err != nil {
		return nil, err
	}
	return list, nil
}

// ListNodesByIDs 批量查询节点（用于 N+1 优化）
func (r *ForwardRepo) ListNodesByIDs(ids []int64) ([]model.Node, error) {
	if len(ids) == 0 {
		return nil, nil
	}
	q, args, err := sqlx.In(`SELECT id, name, secret, server_ip, port, interface_name, version,
		http, tls, socks, created_time, updated_time, status, tcp_listen_addr, udp_listen_addr
		FROM node WHERE id IN (?)`, ids)
	if err != nil {
		return nil, err
	}
	q = r.DB.Rebind(q)
	var list []model.Node
	if err := r.DB.Select(&list, q, args...); err != nil {
		return nil, err
	}
	return list, nil
}

// DeleteByIDs 批量删除转发（分片单语句 IN，替代逐条 Delete）
func (r *ForwardRepo) DeleteByIDs(ids []int64) error {
	if len(ids) == 0 {
		return nil
	}
	for _, chunk := range chunkInt64IDs(ids, 500) {
		q, args, err := sqlx.In(`DELETE FROM forward WHERE id IN (?)`, chunk)
		if err != nil {
			return err
		}
		q = r.DB.Rebind(q)
		if _, err := r.DB.Exec(q, args...); err != nil {
			return err
		}
	}
	return nil
}

// UpdateStatusByIDs 批量更新转发状态（分片单语句 IN，替代逐条 UpdateStatus）
func (r *ForwardRepo) UpdateStatusByIDs(ids []int64, status int, updatedTime int64) error {
	if len(ids) == 0 {
		return nil
	}
	for _, chunk := range chunkInt64IDs(ids, 500) {
		q, args, err := sqlx.In(`UPDATE forward SET status=?, updated_time=? WHERE id IN (?)`, status, updatedTime, chunk)
		if err != nil {
			return err
		}
		q = r.DB.Rebind(q)
		if _, err := r.DB.Exec(q, args...); err != nil {
			return err
		}
	}
	return nil
}

// UpdateInxBatch 单语句 CASE WHEN 批量更新排序（替代逐条 UpdateInx）
func (r *ForwardRepo) UpdateInxBatch(inxByID map[int64]int) error {
	if len(inxByID) == 0 {
		return nil
	}
	ids := make([]int64, 0, len(inxByID))
	for id := range inxByID {
		ids = append(ids, id)
	}
	sort.Slice(ids, func(i, j int) bool { return ids[i] < ids[j] })
	// 每行占 3 个变量（WHEN/THEN/IN），分片 250 → 750 变量 < 999 上限
	for _, chunk := range chunkInt64IDs(ids, 250) {
		var sb strings.Builder
		args := make([]any, 0, len(chunk)*3)
		sb.WriteString(`UPDATE forward SET inx = CASE id `)
		for _, id := range chunk {
			sb.WriteString(`WHEN ? THEN ? `)
			args = append(args, id, inxByID[id])
		}
		sb.WriteString(`END WHERE id IN (`)
		for i, id := range chunk {
			if i > 0 {
				sb.WriteString(`,`)
			}
			sb.WriteString(`?`)
			args = append(args, id)
		}
		sb.WriteString(`)`)
		if _, err := r.DB.Exec(sb.String(), args...); err != nil {
			return err
		}
	}
	return nil
}

func (r *ForwardRepo) ListByTunnelID(tunnelID int64) ([]model.Forward, error) {
	var list []model.Forward
	err := r.DB.Select(&list, `SELECT id, user_id, user_name, name, tunnel_id, remote_addr, strategy,
		in_flow, out_flow, created_time, updated_time, status, inx FROM forward WHERE tunnel_id=?`, tunnelID)
	return list, err
}

func (r *ForwardRepo) CountByTunnelID(tunnelID int64) (int64, error) {
	var n int64
	err := r.DB.Get(&n, `SELECT COUNT(1) FROM forward WHERE tunnel_id=?`, tunnelID)
	return n, err
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

// ListEntriesByTunnelIDs 批量查询多个隧道的入口节点（批量操作预取，避免 N+1）
func (r *ForwardRepo) ListEntriesByTunnelIDs(tunnelIDs []int64) ([]model.ChainTunnel, error) {
	if len(tunnelIDs) == 0 {
		return nil, nil
	}
	q, args, err := sqlx.In(`SELECT id, tunnel_id, chain_type, node_id, port, strategy, inx, protocol
		FROM chain_tunnel WHERE tunnel_id IN (?) AND chain_type = '1' ORDER BY id`, tunnelIDs)
	if err != nil {
		return nil, err
	}
	q = r.DB.Rebind(q)
	var list []model.ChainTunnel
	if err := r.DB.Select(&list, q, args...); err != nil {
		return nil, err
	}
	return list, nil
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
