package repo

import (
	"database/sql"
	"fmt"
	"strings"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/jmoiron/sqlx"
)

type UserRepo struct {
	DB *sqlx.DB
}

func NewUserRepo(db *sqlx.DB) *UserRepo {
	return &UserRepo{DB: db}
}

func (r *UserRepo) FindByUsername(username string) (*model.User, error) {
	var u model.User
	err := r.DB.Get(&u, `SELECT * FROM user WHERE user = ? LIMIT 1`, username)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &u, nil
}

func (r *UserRepo) FindByID(id int64) (*model.User, error) {
	var u model.User
	err := r.DB.Get(&u, `SELECT * FROM user WHERE id = ? LIMIT 1`, id)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &u, nil
}

func (r *UserRepo) CountByUsername(username string, excludeID int64) (int, error) {
	var n int
	var err error
	if excludeID > 0 {
		err = r.DB.Get(&n, `SELECT COUNT(1) FROM user WHERE user = ? AND id <> ?`, username, excludeID)
	} else {
		err = r.DB.Get(&n, `SELECT COUNT(1) FROM user WHERE user = ?`, username)
	}
	return n, err
}

func (r *UserRepo) ListNonAdmin() ([]model.User, error) {
	var list []model.User
	err := r.DB.Select(&list, `SELECT * FROM user WHERE role_id <> 0 ORDER BY id`)
	if err != nil {
		return nil, err
	}
	if list == nil {
		list = []model.User{}
	}
	return list, nil
}

func (r *UserRepo) Create(u *model.User) error {
	res, err := r.DB.Exec(`
		INSERT INTO user (user, pwd, role_id, exp_time, flow, in_flow, out_flow, flow_reset_time, num, created_time, updated_time, status)
		VALUES (?, ?, ?, ?, ?, 0, 0, ?, ?, ?, ?, ?)`,
		u.User, u.Pwd, u.RoleID, u.ExpTime, u.Flow, u.FlowResetTime, u.Num, u.CreatedTime, u.UpdatedTime, u.Status,
	)
	if err != nil {
		return err
	}
	id, _ := res.LastInsertId()
	u.ID = id
	return nil
}

func (r *UserRepo) Update(u *model.User, updatePwd bool) error {
	if updatePwd {
		_, err := r.DB.Exec(`
			UPDATE user SET user=?, pwd=?, flow=?, num=?, exp_time=?, flow_reset_time=?, status=?, updated_time=?
			WHERE id=?`,
			u.User, u.Pwd, u.Flow, u.Num, u.ExpTime, u.FlowResetTime, u.Status, u.UpdatedTime, u.ID,
		)
		return err
	}
	_, err := r.DB.Exec(`
		UPDATE user SET user=?, flow=?, num=?, exp_time=?, flow_reset_time=?, status=?, updated_time=?
		WHERE id=?`,
		u.User, u.Flow, u.Num, u.ExpTime, u.FlowResetTime, u.Status, u.UpdatedTime, u.ID,
	)
	return err
}

func (r *UserRepo) UpdatePasswordAndUsername(id int64, username, pwdMD5 string) error {
	now := time.Now().UnixMilli()
	_, err := r.DB.Exec(`UPDATE user SET user=?, pwd=?, updated_time=? WHERE id=?`, username, pwdMD5, now, id)
	return err
}

func (r *UserRepo) ResetUserFlow(id int64) error {
	_, err := r.DB.Exec(`UPDATE user SET in_flow=0, out_flow=0, updated_time=? WHERE id=?`, time.Now().UnixMilli(), id)
	return err
}

func (r *UserRepo) Delete(id int64) error {
	_, err := r.DB.Exec(`DELETE FROM user WHERE id=?`, id)
	return err
}

func (r *UserRepo) DeleteUserTunnels(userID int64) error {
	_, err := r.DB.Exec(`DELETE FROM user_tunnel WHERE user_id=?`, userID)
	return err
}

func (r *UserRepo) DeleteStatisticsFlow(userID int64) error {
	_, err := r.DB.Exec(`DELETE FROM statistics_flow WHERE user_id=?`, userID)
	return err
}

func (r *UserRepo) ListForwardIDsByUser(userID int64) ([]int64, error) {
	var ids []int64
	err := r.DB.Select(&ids, `SELECT id FROM forward WHERE user_id=?`, userID)
	return ids, err
}

func (r *UserRepo) DeleteForwardsByUser(userID int64) error {
	tx, err := r.DB.Beginx()
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()

	var ids []int64
	if err := tx.Select(&ids, `SELECT id FROM forward WHERE user_id=?`, userID); err != nil {
		return err
	}
	if len(ids) > 0 {
		query, args, err := sqlx.In(`DELETE FROM forward_port WHERE forward_id IN (?)`, ids)
		if err != nil {
			return err
		}
		query = tx.Rebind(query)
		if _, err := tx.Exec(query, args...); err != nil {
			return err
		}
	}
	if _, err := tx.Exec(`DELETE FROM forward WHERE user_id=?`, userID); err != nil {
		return err
	}
	return tx.Commit()
}

// ---- package info ----

// PackageUserTunnelDetail 用户套餐页隧道明细（与 UserTunnelDetail 字段略有差异）
type PackageUserTunnelDetail struct {
	ID             int     `db:"id" json:"id"`
	UserID         int     `db:"user_id" json:"userId"`
	TunnelID       int     `db:"tunnel_id" json:"tunnelId"`
	TunnelName     *string `db:"tunnel_name" json:"tunnelName"`
	TunnelFlow     *int    `db:"tunnel_flow" json:"tunnelFlow"`
	Flow           int64   `db:"flow" json:"flow"`
	InFlow         int64   `db:"in_flow" json:"inFlow"`
	OutFlow        int64   `db:"out_flow" json:"outFlow"`
	Num            int     `db:"num" json:"num"`
	FlowResetTime  int64   `db:"flow_reset_time" json:"flowResetTime"`
	ExpTime        int64   `db:"exp_time" json:"expTime"`
	SpeedID        *int    `db:"speed_id" json:"speedId"`
	SpeedLimitName *string `db:"speed_limit_name" json:"speedLimitName"`
	Speed          *int    `db:"speed" json:"speed"`
}

type UserForwardDetail struct {
	ID          int64   `db:"id" json:"id"`
	Name        string  `db:"name" json:"name"`
	TunnelID    int     `db:"tunnel_id" json:"tunnelId"`
	TunnelName  *string `db:"tunnel_name" json:"tunnelName"`
	InIP        *string `db:"in_ip" json:"inIp"`
	InPort      *int    `json:"inPort"`
	RemoteAddr  string  `db:"remote_addr" json:"remoteAddr"`
	InFlow      int64   `db:"in_flow" json:"inFlow"`
	OutFlow     int64   `db:"out_flow" json:"outFlow"`
	Status      int     `db:"status" json:"status"`
	CreatedTime int64   `db:"created_time" json:"createdTime"`
}

func (r *UserRepo) GetUserTunnelDetails(userID int64) ([]PackageUserTunnelDetail, error) {
	var list []PackageUserTunnelDetail
	err := r.DB.Select(&list, `
		SELECT
			ut.id,
			ut.user_id,
			ut.tunnel_id,
			t.name as tunnel_name,
			t.flow as tunnel_flow,
			ut.flow,
			ut.in_flow,
			ut.out_flow,
			ut.num,
			ut.flow_reset_time,
			ut.exp_time,
			ut.speed_id,
			sl.name as speed_limit_name,
			sl.speed
		FROM user_tunnel ut
		LEFT JOIN tunnel t ON ut.tunnel_id = t.id
		LEFT JOIN speed_limit sl ON ut.speed_id = sl.id
		WHERE ut.user_id = ?
		ORDER BY ut.id`, userID)
	if err != nil {
		return nil, err
	}
	if list == nil {
		list = []PackageUserTunnelDetail{}
	}
	return list, nil
}

func (r *UserRepo) GetUserForwardDetails(userID int64) ([]UserForwardDetail, error) {
	var list []UserForwardDetail
	err := r.DB.Select(&list, `
		SELECT
			f.id,
			f.name,
			f.tunnel_id,
			t.name as tunnel_name,
			t.in_ip as in_ip,
			f.remote_addr,
			f.in_flow,
			f.out_flow,
			f.status,
			f.created_time
		FROM forward f
		LEFT JOIN tunnel t ON f.tunnel_id = t.id
		WHERE f.user_id = ?
		ORDER BY f.created_time DESC`, userID)
	if err != nil {
		return nil, err
	}
	if list == nil {
		list = []UserForwardDetail{}
	}
	return list, nil
}

type forwardPortRow struct {
	ForwardID int64  `db:"forward_id"`
	NodeID    int64  `db:"node_id"`
	Port      int    `db:"port"`
	ServerIP  string `db:"server_ip"`
}

func (r *UserRepo) FillForwardInIPAndPort(forwards []UserForwardDetail) error {
	if len(forwards) == 0 {
		return nil
	}
	ids := make([]int64, 0, len(forwards))
	for _, f := range forwards {
		ids = append(ids, f.ID)
	}
	query, args, err := sqlx.In(`
		SELECT fp.forward_id, fp.node_id, fp.port, IFNULL(n.server_ip,'') as server_ip
		FROM forward_port fp
		LEFT JOIN node n ON fp.node_id = n.id
		WHERE fp.forward_id IN (?)
		ORDER BY fp.id`, ids)
	if err != nil {
		return err
	}
	query = r.DB.Rebind(query)
	var ports []forwardPortRow
	if err := r.DB.Select(&ports, query, args...); err != nil {
		return err
	}
	byFwd := map[int64][]forwardPortRow{}
	for _, p := range ports {
		byFwd[p.ForwardID] = append(byFwd[p.ForwardID], p)
	}

	for i := range forwards {
		f := &forwards[i]
		fps := byFwd[f.ID]
		if len(fps) == 0 {
			continue
		}
		useTunnelInIP := f.InIP != nil && strings.TrimSpace(*f.InIP) != ""
		ipPortSet := make([]string, 0)
		seen := map[string]struct{}{}
		add := func(s string) {
			if _, ok := seen[s]; ok {
				return
			}
			seen[s] = struct{}{}
			ipPortSet = append(ipPortSet, s)
		}

		if useTunnelInIP {
			ipList := []string{}
			for _, part := range strings.Split(*f.InIP, ",") {
				part = strings.TrimSpace(part)
				if part != "" {
					ipList = append(ipList, part)
				}
			}
			// unique ips
			uniqIP := []string{}
			ipSeen := map[string]struct{}{}
			for _, ip := range ipList {
				if _, ok := ipSeen[ip]; !ok {
					ipSeen[ip] = struct{}{}
					uniqIP = append(uniqIP, ip)
				}
			}
			uniqPorts := []int{}
			portSeen := map[int]struct{}{}
			for _, p := range fps {
				if _, ok := portSeen[p.Port]; !ok {
					portSeen[p.Port] = struct{}{}
					uniqPorts = append(uniqPorts, p.Port)
				}
			}
			for _, ip := range uniqIP {
				for _, port := range uniqPorts {
					add(fmt.Sprintf("%s:%d", ip, port))
				}
			}
			if len(uniqPorts) > 0 {
				p := uniqPorts[0]
				f.InPort = &p
			}
		} else {
			for _, p := range fps {
				if p.ServerIP != "" {
					add(fmt.Sprintf("%s:%d", p.ServerIP, p.Port))
				}
			}
			p := fps[0].Port
			f.InPort = &p
		}
		if len(ipPortSet) > 0 {
			joined := strings.Join(ipPortSet, ",")
			f.InIP = &joined
		}
	}
	return nil
}

func (r *UserRepo) GetLast24HoursFlow(userID int64) ([]model.StatisticsFlow, error) {
	var list []model.StatisticsFlow
	err := r.DB.Select(&list, `
		SELECT * FROM statistics_flow
		WHERE user_id = ?
		ORDER BY id DESC
		LIMIT 24`, userID)
	if err != nil {
		return nil, err
	}
	if list == nil {
		list = []model.StatisticsFlow{}
	}
	// 补齐 24 条
	if len(list) < 24 {
		startHour := time.Now().Hour()
		if len(list) > 0 {
			startHour = parseHour(list[len(list)-1].Time) - 1
		}
		for len(list) < 24 {
			if startHour < 0 {
				startHour = 23
			}
			list = append(list, model.StatisticsFlow{
				UserID:    userID,
				Flow:      0,
				TotalFlow: 0,
				Time:      fmt.Sprintf("%02d:00", startHour),
			})
			startHour--
		}
	}
	return list, nil
}

func parseHour(timeStr string) int {
	if timeStr != "" && strings.Contains(timeStr, ":") {
		var h int
		if _, err := fmt.Sscanf(timeStr, "%d:", &h); err == nil {
			return h
		}
	}
	return time.Now().Hour()
}

func (r *UserRepo) FindUserTunnelByID(id int64) (*model.UserTunnel, error) {
	var ut model.UserTunnel
	err := r.DB.Get(&ut, `SELECT * FROM user_tunnel WHERE id = ? LIMIT 1`, id)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &ut, nil
}

func (r *UserRepo) ResetUserTunnelFlow(id int64) error {
	_, err := r.DB.Exec(`UPDATE user_tunnel SET in_flow=0, out_flow=0 WHERE id=?`, id)
	return err
}
