package task

import (
	"log/slog"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/db"
	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/service"
	"github.com/Fearless743/flux-panel/go-backend/internal/ws"
	"github.com/jmoiron/sqlx"
	"github.com/robfig/cron/v3"
)

// Scheduler 定时任务
type Scheduler struct {
	DB   *sqlx.DB
	Hub  *ws.Hub
	cron *cron.Cron
	flow *service.FlowService
}

func New(database *sqlx.DB, hub *ws.Hub) *Scheduler {
	return &Scheduler{
		DB:   database,
		Hub:  hub,
		cron: cron.New(cron.WithSeconds()),
		flow: service.NewFlowService(database, hub),
	}
}

// Start 注册并启动 cron
func (s *Scheduler) Start() {
	// ResetFlow: 每天 00:00:05
	_, _ = s.cron.AddFunc("5 0 0 * * ?", func() {
		s.resetFlow()
	})
	// StatisticsFlow: 每小时整点
	_, _ = s.cron.AddFunc("0 0 * * * ?", func() {
		s.statisticsFlow()
	})
	// WAL checkpoint 每 5 分钟
	_, _ = s.cron.AddFunc("0 */5 * * * ?", func() {
		db.Checkpoint(s.DB)
	})
	s.cron.Start()
	slog.Info("cron started", "jobs", "ResetFlow/StatisticsFlow/WAL")
}

func (s *Scheduler) Stop() {
	if s.cron != nil {
		ctx := s.cron.Stop()
		<-ctx.Done()
	}
}

func (s *Scheduler) resetFlow() {
	slog.Info("开始执行流量重置任务")
	now := time.Now()
	currentDay := now.Day()
	lastDay := time.Date(now.Year(), now.Month()+1, 0, 0, 0, 0, 0, now.Location()).Day()

	s.resetUserFlow(currentDay, lastDay)
	s.resetUserTunnelFlow(currentDay, lastDay)
	slog.Info("流量重置任务执行完成")

	s.expireUsers()
	s.expireUserTunnels()
	slog.Info("到期任务执行完成")
}

func (s *Scheduler) resetUserFlow(currentDay, lastDay int) {
	var users []model.User
	var err error
	if currentDay == lastDay {
		err = s.DB.Select(&users, `
			SELECT * FROM user
			WHERE flow_reset_time != 0
			  AND (flow_reset_time = ? OR flow_reset_time > ?)`, currentDay, lastDay)
	} else {
		err = s.DB.Select(&users, `
			SELECT * FROM user
			WHERE flow_reset_time != 0 AND flow_reset_time = ?`, currentDay)
	}
	if err != nil {
		slog.Warn("查询需重置用户失败", "err", err)
		return
	}
	if len(users) == 0 {
		slog.Info("没有需要重置流量的用户")
		return
	}
	slog.Info("找到需要重置流量的用户", "count", len(users))
	for _, u := range users {
		if _, err := s.DB.Exec(`UPDATE user SET in_flow = 0, out_flow = 0 WHERE id = ?`, u.ID); err != nil {
			slog.Warn("用户流量重置失败", "id", u.ID, "err", err)
		} else {
			slog.Info("用户流量重置成功", "id", u.ID, "user", u.User, "day", u.FlowResetTime)
		}
	}
}

func (s *Scheduler) resetUserTunnelFlow(currentDay, lastDay int) {
	var list []model.UserTunnel
	var err error
	if currentDay == lastDay {
		err = s.DB.Select(&list, `
			SELECT * FROM user_tunnel
			WHERE flow_reset_time != 0
			  AND (flow_reset_time = ? OR flow_reset_time > ?)`, currentDay, lastDay)
	} else {
		err = s.DB.Select(&list, `
			SELECT * FROM user_tunnel
			WHERE flow_reset_time != 0 AND flow_reset_time = ?`, currentDay)
	}
	if err != nil {
		slog.Warn("查询需重置用户隧道失败", "err", err)
		return
	}
	if len(list) == 0 {
		slog.Info("没有需要重置流量的用户隧道")
		return
	}
	slog.Info("找到需要重置流量的用户隧道", "count", len(list))
	for _, ut := range list {
		if _, err := s.DB.Exec(`UPDATE user_tunnel SET in_flow = 0, out_flow = 0 WHERE id = ?`, ut.ID); err != nil {
			slog.Warn("用户隧道流量重置失败", "id", ut.ID, "err", err)
		} else {
			slog.Info("用户隧道流量重置成功", "id", ut.ID, "user", ut.UserID, "tunnel", ut.TunnelID)
		}
	}
}

func (s *Scheduler) expireUsers() {
	now := time.Now().UnixMilli()
	var users []model.User
	if err := s.DB.Select(&users, `
		SELECT * FROM user
		WHERE role_id != 0 AND status = 1 AND exp_time IS NOT NULL AND exp_time < ?`, now); err != nil {
		slog.Warn("查询过期用户失败", "err", err)
		return
	}
	for _, u := range users {
		var forwards []model.Forward
		_ = s.DB.Select(&forwards, `SELECT * FROM forward WHERE user_id = ? AND status = 1`, u.ID)
		s.flow.PauseForwardByIDs(forwards)
		_, _ = s.DB.Exec(`UPDATE user SET status = 0 WHERE id = ?`, u.ID)
	}
}

func (s *Scheduler) expireUserTunnels() {
	now := time.Now().UnixMilli()
	var list []model.UserTunnel
	if err := s.DB.Select(&list, `
		SELECT * FROM user_tunnel
		WHERE status = 1 AND exp_time IS NOT NULL AND exp_time < ?`, now); err != nil {
		slog.Warn("查询过期隧道失败", "err", err)
		return
	}
	for _, ut := range list {
		var forwards []model.Forward
		_ = s.DB.Select(&forwards, `
			SELECT * FROM forward
			WHERE tunnel_id = ? AND user_id = ? AND status = 1`, ut.TunnelID, ut.UserID)
		s.flow.PauseForwardByIDs(forwards)
		_, _ = s.DB.Exec(`UPDATE user_tunnel SET status = 0 WHERE id = ?`, ut.ID)
	}
}

func (s *Scheduler) statisticsFlow() {
	now := time.Now()
	hourString := now.Truncate(time.Hour).Format("15:04")
	ts := now.UnixMilli()
	cutoff := ts - 48*60*60*1000

	if _, err := s.DB.Exec(`DELETE FROM statistics_flow WHERE created_time < ?`, cutoff); err != nil {
		slog.Warn("清理统计流量失败", "err", err)
	}

	var users []model.User
	if err := s.DB.Select(&users, `SELECT * FROM user`); err != nil {
		slog.Warn("统计流量查询用户失败", "err", err)
		return
	}

	for _, u := range users {
		currentTotal := u.InFlow + u.OutFlow
		var last model.StatisticsFlow
		err := s.DB.Get(&last, `
			SELECT * FROM statistics_flow
			WHERE user_id = ?
			ORDER BY id DESC LIMIT 1`, u.ID)
		increment := currentTotal
		if err == nil {
			increment = currentTotal - last.TotalFlow
			if increment < 0 {
				increment = currentTotal
			}
		}
		_, err = s.DB.Exec(`
			INSERT INTO statistics_flow (user_id, flow, total_flow, time, created_time)
			VALUES (?, ?, ?, ?, ?)`,
			u.ID, increment, currentTotal, hourString, ts)
		if err != nil {
			slog.Warn("写入统计流量失败", "user", u.ID, "err", err)
		}
	}
}
