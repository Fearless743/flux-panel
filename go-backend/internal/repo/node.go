package repo

import (
	"database/sql"
	"fmt"
	"strings"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/jmoiron/sqlx"
)

type NodeRepo struct {
	DB *sqlx.DB
}

func NewNodeRepo(db *sqlx.DB) *NodeRepo {
	return &NodeRepo{DB: db}
}

func nullListen(s string) string {
	if s == "" {
		return "[::]"
	}
	return s
}

func (r *NodeRepo) Create(n *model.Node) error {
	now := time.Now().UnixMilli()
	n.CreatedTime = now
	n.UpdatedTime = &now
	res, err := r.DB.Exec(`
		INSERT INTO node (name, secret, server_ip, port, interface_name, version, http, tls, socks,
			created_time, updated_time, status, tcp_listen_addr, udp_listen_addr)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		n.Name, n.Secret, n.ServerIP, n.Port, n.InterfaceName, n.Version,
		n.HTTP, n.TLS, n.Socks, n.CreatedTime, n.UpdatedTime, n.Status,
		nullListen(n.TCPListenAddr), nullListen(n.UDPListenAddr),
	)
	if err != nil {
		return err
	}
	id, err := res.LastInsertId()
	if err != nil {
		return err
	}
	n.ID = id
	return nil
}

func (r *NodeRepo) GetByID(id int64) (*model.Node, error) {
	var n model.Node
	err := r.DB.Get(&n, `SELECT * FROM node WHERE id = ?`, id)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &n, nil
}

func (r *NodeRepo) GetBySecret(secret string) (*model.Node, error) {
	var n model.Node
	err := r.DB.Get(&n, `SELECT * FROM node WHERE secret = ? LIMIT 1`, secret)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &n, nil
}

func (r *NodeRepo) ListOrderByStatusDesc() ([]model.Node, error) {
	var list []model.Node
	err := r.DB.Select(&list, `SELECT * FROM node ORDER BY status DESC, id DESC`)
	if err != nil {
		return nil, err
	}
	if list == nil {
		list = []model.Node{}
	}
	return list, nil
}

func (r *NodeRepo) ListByIDs(ids []int64) ([]model.Node, error) {
	if len(ids) == 0 {
		return nil, nil
	}
	q, args, err := sqlx.In(`SELECT * FROM node WHERE id IN (?)`, ids)
	if err != nil {
		return nil, err
	}
	q = r.DB.Rebind(q)
	var list []model.Node
	err = r.DB.Select(&list, q, args...)
	return list, err
}

func (r *NodeRepo) MapByIDs(ids []int64) (map[int64]model.Node, error) {
	list, err := r.ListByIDs(ids)
	if err != nil {
		return nil, err
	}
	m := make(map[int64]model.Node, len(list))
	for _, n := range list {
		m[n.ID] = n
	}
	return m, nil
}

func (r *NodeRepo) Update(n *model.Node) error {
	now := time.Now().UnixMilli()
	n.UpdatedTime = &now
	_, err := r.DB.Exec(`
		UPDATE node SET name=?, server_ip=?, port=?, interface_name=?,
			http=?, tls=?, socks=?, updated_time=?, tcp_listen_addr=?, udp_listen_addr=?
		WHERE id=?`,
		n.Name, n.ServerIP, n.Port, n.InterfaceName,
		n.HTTP, n.TLS, n.Socks, n.UpdatedTime,
		nullListen(n.TCPListenAddr), nullListen(n.UDPListenAddr), n.ID,
	)
	return err
}

func (r *NodeRepo) UpdateOnline(id int64, version *string, http, tls, socks *int) error {
	now := time.Now().UnixMilli()
	sets := []string{"status = 1", "updated_time = ?"}
	args := []any{now}
	if version != nil {
		sets = append(sets, "version = ?")
		args = append(args, *version)
	}
	if http != nil {
		sets = append(sets, "http = ?")
		args = append(args, *http)
	}
	if tls != nil {
		sets = append(sets, "tls = ?")
		args = append(args, *tls)
	}
	if socks != nil {
		sets = append(sets, "socks = ?")
		args = append(args, *socks)
	}
	args = append(args, id)
	q := `UPDATE node SET ` + strings.Join(sets, ", ") + ` WHERE id = ?`
	_, err := r.DB.Exec(q, args...)
	return err
}

func (r *NodeRepo) UpdateStatus(id int64, status int) error {
	now := time.Now().UnixMilli()
	_, err := r.DB.Exec(`UPDATE node SET status = ?, updated_time = ? WHERE id = ?`, status, now, id)
	return err
}

func (r *NodeRepo) Delete(id int64) error {
	res, err := r.DB.Exec(`DELETE FROM node WHERE id=?`, id)
	if err != nil {
		return err
	}
	n, err := res.RowsAffected()
	if err != nil {
		return err
	}
	if n == 0 {
		return fmt.Errorf("节点不存在")
	}
	return nil
}

func (r *NodeRepo) GetViteConfig(name string) (string, error) {
	var value string
	err := r.DB.Get(&value, `SELECT value FROM vite_config WHERE name = ? LIMIT 1`, name)
	if err == sql.ErrNoRows {
		return "", nil
	}
	if err != nil {
		return "", err
	}
	return value, nil
}
