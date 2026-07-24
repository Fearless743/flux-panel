package repo

import (
	"database/sql"
	"fmt"

	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/jmoiron/sqlx"
)

type TunnelRepo struct {
	DB *sqlx.DB
}

func NewTunnelRepo(db *sqlx.DB) *TunnelRepo {
	return &TunnelRepo{DB: db}
}

func (r *TunnelRepo) CountByName(name string) (int, error) {
	var n int
	err := r.DB.Get(&n, `SELECT COUNT(1) FROM tunnel WHERE name = ?`, name)
	return n, err
}

func (r *TunnelRepo) GetByID(id int64) (*model.Tunnel, error) {
	var t model.Tunnel
	err := r.DB.Get(&t, `SELECT * FROM tunnel WHERE id = ?`, id)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &t, nil
}

func (r *TunnelRepo) ListAll() ([]model.Tunnel, error) {
	var list []model.Tunnel
	err := r.DB.Select(&list, `SELECT * FROM tunnel ORDER BY id`)
	return list, err
}

func (r *TunnelRepo) ListByIDs(ids []int64) ([]model.Tunnel, error) {
	if len(ids) == 0 {
		return nil, nil
	}
	q, args, err := sqlx.In(`SELECT * FROM tunnel WHERE id IN (?) AND status = 1`, ids)
	if err != nil {
		return nil, err
	}
	q = r.DB.Rebind(q)
	var list []model.Tunnel
	err = r.DB.Select(&list, q, args...)
	return list, err
}

func (r *TunnelRepo) ListEnabled() ([]model.Tunnel, error) {
	var list []model.Tunnel
	err := r.DB.Select(&list, `SELECT * FROM tunnel WHERE status = 1 ORDER BY id`)
	return list, err
}

func (r *TunnelRepo) Insert(t *model.Tunnel) error {
	res, err := r.DB.Exec(`
		INSERT INTO tunnel (name, traffic_ratio, type, protocol, flow, created_time, updated_time, status, in_ip)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		t.Name, t.TrafficRatio, t.Type, t.Protocol, t.Flow, t.CreatedTime, t.UpdatedTime, t.Status, t.InIP,
	)
	if err != nil {
		return err
	}
	id, err := res.LastInsertId()
	if err != nil {
		return err
	}
	t.ID = id
	return nil
}

func (r *TunnelRepo) UpdateBasic(id int64, name string, flow int, trafficRatio float64, inIP *string, updatedTime int64) error {
	_, err := r.DB.Exec(`
		UPDATE tunnel SET name = ?, flow = ?, traffic_ratio = ?, in_ip = ?, updated_time = ?
		WHERE id = ?`,
		name, flow, trafficRatio, inIP, updatedTime, id,
	)
	return err
}

func (r *TunnelRepo) UpdateInIP(id int64, inIP string, updatedTime int64) error {
	_, err := r.DB.Exec(`UPDATE tunnel SET in_ip = ?, updated_time = ? WHERE id = ?`, inIP, updatedTime, id)
	return err
}

func (r *TunnelRepo) Delete(id int64) error {
	_, err := r.DB.Exec(`DELETE FROM tunnel WHERE id = ?`, id)
	return err
}

func (r *TunnelRepo) DeleteWithTx(tx *sqlx.Tx, id int64) error {
	_, err := tx.Exec(`DELETE FROM tunnel WHERE id = ?`, id)
	return err
}

func strPtr(s string) *string {
	if s == "" {
		return nil
	}
	return &s
}

func fmtErr(msg string, err error) error {
	if err == nil {
		return nil
	}
	return fmt.Errorf("%s: %w", msg, err)
}
