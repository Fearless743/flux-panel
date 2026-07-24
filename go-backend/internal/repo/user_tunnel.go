package repo

import (
	"database/sql"

	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/jmoiron/sqlx"
)

type UserTunnelRepo struct {
	DB *sqlx.DB
}

func NewUserTunnelRepo(db *sqlx.DB) *UserTunnelRepo {
	return &UserTunnelRepo{DB: db}
}

type UserTunnelDetail struct {
	ID            int     `db:"id" json:"id"`
	UserID        int     `db:"user_id" json:"userId"`
	TunnelID      int     `db:"tunnel_id" json:"tunnelId"`
	Flow          int64   `db:"flow" json:"flow"`
	InFlow        int64   `db:"in_flow" json:"inFlow"`
	OutFlow       int64   `db:"out_flow" json:"outFlow"`
	Num           int     `db:"num" json:"num"`
	FlowResetTime int64   `db:"flow_reset_time" json:"flowResetTime"`
	ExpTime       int64   `db:"exp_time" json:"expTime"`
	SpeedID       *int    `db:"speed_id" json:"speedId"`
	Status        int     `db:"status" json:"status"`
	TunnelName    *string `db:"tunnel_name" json:"tunnelName"`
	TunnelFlow    *int    `db:"tunnel_flow" json:"tunnelFlow"`
	InIP          *string `db:"in_ip" json:"inIp"`
	Type          *int    `db:"type" json:"type"`
	Protocol      *string `db:"protocol" json:"protocol"`
	SpeedLimitName *string `db:"speed_limit_name" json:"speedLimitName"`
	Speed         *int    `db:"speed" json:"speed"`
}

func (r *UserTunnelRepo) CountByUserAndTunnel(userID, tunnelID int) (int, error) {
	var n int
	err := r.DB.Get(&n, `SELECT COUNT(1) FROM user_tunnel WHERE user_id = ? AND tunnel_id = ?`, userID, tunnelID)
	return n, err
}

func (r *UserTunnelRepo) GetByID(id int) (*model.UserTunnel, error) {
	var ut model.UserTunnel
	err := r.DB.Get(&ut, `SELECT * FROM user_tunnel WHERE id = ?`, id)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &ut, nil
}

func (r *UserTunnelRepo) GetByUserAndTunnel(userID, tunnelID int) (*model.UserTunnel, error) {
	var ut model.UserTunnel
	err := r.DB.Get(&ut, `SELECT * FROM user_tunnel WHERE user_id = ? AND tunnel_id = ? LIMIT 1`, userID, tunnelID)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &ut, nil
}

func (r *UserTunnelRepo) ListByUserID(userID int) ([]model.UserTunnel, error) {
	var list []model.UserTunnel
	err := r.DB.Select(&list, `SELECT * FROM user_tunnel WHERE user_id = ? ORDER BY id`, userID)
	return list, err
}

func (r *UserTunnelRepo) ListDetailsByUserID(userID int) ([]UserTunnelDetail, error) {
	var list []UserTunnelDetail
	err := r.DB.Select(&list, `
		SELECT
			ut.id,
			ut.user_id,
			ut.tunnel_id,
			ut.flow,
			ut.in_flow,
			ut.out_flow,
			ut.num,
			ut.flow_reset_time,
			ut.exp_time,
			ut.speed_id,
			ut.status,
			t.name AS tunnel_name,
			t.flow AS tunnel_flow,
			t.in_ip,
			t.type,
			t.protocol,
			sl.name AS speed_limit_name,
			sl.speed
		FROM user_tunnel ut
		LEFT JOIN tunnel t ON ut.tunnel_id = t.id
		LEFT JOIN speed_limit sl ON ut.speed_id = sl.id
		WHERE ut.user_id = ?
		ORDER BY ut.id`, userID)
	return list, err
}

func (r *UserTunnelRepo) Insert(ut *model.UserTunnel) error {
	res, err := r.DB.Exec(`
		INSERT INTO user_tunnel (user_id, tunnel_id, speed_id, num, flow, in_flow, out_flow, flow_reset_time, exp_time, status)
		VALUES (?, ?, ?, ?, ?, 0, 0, ?, ?, ?)`,
		ut.UserID, ut.TunnelID, ut.SpeedID, ut.Num, ut.Flow, ut.FlowResetTime, ut.ExpTime, ut.Status,
	)
	if err != nil {
		return err
	}
	id, err := res.LastInsertId()
	if err != nil {
		return err
	}
	ut.ID = int(id)
	return nil
}

func (r *UserTunnelRepo) Update(ut *model.UserTunnel) error {
	_, err := r.DB.Exec(`
		UPDATE user_tunnel
		SET flow = ?, num = ?, flow_reset_time = ?, exp_time = ?, status = ?, speed_id = ?
		WHERE id = ?`,
		ut.Flow, ut.Num, ut.FlowResetTime, ut.ExpTime, ut.Status, ut.SpeedID, ut.ID,
	)
	return err
}

func (r *UserTunnelRepo) Delete(id int) error {
	_, err := r.DB.Exec(`DELETE FROM user_tunnel WHERE id = ?`, id)
	return err
}

func (r *UserTunnelRepo) DeleteByTunnelID(tunnelID int64) error {
	_, err := r.DB.Exec(`DELETE FROM user_tunnel WHERE tunnel_id = ?`, tunnelID)
	return err
}
