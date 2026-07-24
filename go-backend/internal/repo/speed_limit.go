package repo

import (
	"database/sql"
	"errors"

	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/jmoiron/sqlx"
)

type SpeedLimitRepo struct {
	DB *sqlx.DB
}

func NewSpeedLimitRepo(db *sqlx.DB) *SpeedLimitRepo {
	return &SpeedLimitRepo{DB: db}
}

func (r *SpeedLimitRepo) GetTunnelByID(id int64) (*model.Tunnel, error) {
	var t model.Tunnel
	err := r.DB.Get(&t, `SELECT id, name, traffic_ratio, type, protocol, flow, created_time, updated_time, status, in_ip FROM tunnel WHERE id = ?`, id)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &t, nil
}

func (r *SpeedLimitRepo) Create(sl *model.SpeedLimit) (int64, error) {
	res, err := r.DB.Exec(
		`INSERT INTO speed_limit (name, speed, tunnel_id, tunnel_name, created_time, updated_time, status)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		sl.Name, sl.Speed, sl.TunnelID, sl.TunnelName, sl.CreatedTime, sl.UpdatedTime, sl.Status,
	)
	if err != nil {
		return 0, err
	}
	return res.LastInsertId()
}

func (r *SpeedLimitRepo) List() ([]model.SpeedLimit, error) {
	var list []model.SpeedLimit
	err := r.DB.Select(&list, `SELECT id, name, speed, tunnel_id, tunnel_name, created_time, updated_time, status FROM speed_limit`)
	if err != nil {
		return nil, err
	}
	if list == nil {
		list = []model.SpeedLimit{}
	}
	return list, nil
}

func (r *SpeedLimitRepo) GetByID(id int64) (*model.SpeedLimit, error) {
	var sl model.SpeedLimit
	err := r.DB.Get(&sl, `SELECT id, name, speed, tunnel_id, tunnel_name, created_time, updated_time, status FROM speed_limit WHERE id = ?`, id)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &sl, nil
}

func (r *SpeedLimitRepo) Update(sl *model.SpeedLimit) error {
	_, err := r.DB.Exec(
		`UPDATE speed_limit SET name = ?, speed = ?, updated_time = ? WHERE id = ?`,
		sl.Name, sl.Speed, sl.UpdatedTime, sl.ID,
	)
	return err
}

func (r *SpeedLimitRepo) Delete(id int64) error {
	_, err := r.DB.Exec(`DELETE FROM speed_limit WHERE id = ?`, id)
	return err
}

// ListChainNodeIDs 返回隧道链上存在的节点 ID（节点表有记录才返回）
func (r *SpeedLimitRepo) ListChainNodeIDs(tunnelID int64) ([]int64, error) {
	var ids []int64
	err := r.DB.Select(&ids, `
		SELECT ct.node_id FROM chain_tunnel ct
		INNER JOIN node n ON n.id = ct.node_id
		WHERE ct.tunnel_id = ?`, tunnelID)
	if err != nil {
		return nil, err
	}
	if ids == nil {
		ids = []int64{}
	}
	return ids, nil
}

func (r *SpeedLimitRepo) CountUserTunnelBySpeedID(speedID int64) (int, error) {
	var n int
	err := r.DB.Get(&n, `SELECT COUNT(1) FROM user_tunnel WHERE speed_id = ?`, speedID)
	return n, err
}

func (r *SpeedLimitRepo) ListAllTunnels() ([]model.Tunnel, error) {
	var list []model.Tunnel
	err := r.DB.Select(&list, `SELECT id, name, traffic_ratio, type, protocol, flow, created_time, updated_time, status, in_ip FROM tunnel`)
	if err != nil {
		return nil, err
	}
	if list == nil {
		list = []model.Tunnel{}
	}
	return list, nil
}
