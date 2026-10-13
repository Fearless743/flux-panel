package task

import (
	"log/slog"
	"strings"
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
	// 单语句批量重置（原先查再逐行 UPDATE，共 N+1 次）
	var query string
	var args []any
	if currentDay == lastDay {
		query = `UPDATE user SET in_flow = 0, out_flow = 0
			WHERE flow_reset_time != 0
			  AND (flow_reset_time = ? OR flow_reset_time > ?)`
		args = []any{currentDay, lastDay}
	} else {
		query = `UPDATE user SET in_flow = 0, out_flow = 0
			WHERE flow_reset_time != 0 AND flow_reset_time = ?`
		args = []any{currentDay}
	}
	res, err := s.DB.Exec(query, args...)
	if err != nil {
		slog.Warn("用户流量批量重置失败", "err", err)
		return
	}
	if n, err := res.RowsAffected(); err == nil {
		if n == 0 {
			slog.Info("没有需要重置流量的用户")
		} else {
			slog.Info("用户流量重置成功", "count", n)
		}
	}
}

func (s *Scheduler) resetUserTunnelFlow(currentDay, lastDay int) {
	// 单语句批量重置（原先查再逐行 UPDATE，共 N+1 次）
	var query string
	var args []any
	if currentDay == lastDay {
		query = `UPDATE user_tunnel SET in_flow = 0, out_flow = 0
			WHERE flow_reset_time != 0
			  AND (flow_reset_time = ? OR flow_reset_time > ?)`
		args = []any{currentDay, lastDay}
	} else {
		query = `UPDATE user_tunnel SET in_flow = 0, out_flow = 0
			WHERE flow_reset_time != 0 AND flow_reset_time = ?`
		args = []any{currentDay}
	}
	res, err := s.DB.Exec(query, args...)
	if err != nil {
		slog.Warn("用户隧道流量批量重置失败", "err", err)
		return
	}
	if n, err := res.RowsAffected(); err == nil {
		if n == 0 {
			slog.Info("没有需要重置流量的用户隧道")
		} else {
			slog.Info("用户隧道流量重置成功", "count", n)
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
	// 转发查询仍按用户逐个（暂停目标不同），暂停下发与状态写批量化
	expiredIDs := make([]int64, 0, len(users))
	for _, u := range users {
		var forwards []model.Forward
		_ = s.DB.Select(&forwards, `SELECT * FROM forward WHERE user_id = ? AND status = 1`, u.ID)
		s.flow.PauseForwardByIDs(forwards)
		expiredIDs = append(expiredIDs, u.ID)
	}
	if len(expiredIDs) > 0 {
		q, args, err := sqlx.In(`UPDATE user SET status = 0 WHERE id IN (?)`, expiredIDs)
		if err == nil {
			q = s.DB.Rebind(q)
			_, _ = s.DB.Exec(q, args...)
		}
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
	expiredIDs := make([]int, 0, len(list))
	for _, ut := range list {
		var forwards []model.Forward
		_ = s.DB.Select(&forwards, `
			SELECT * FROM forward
			WHERE tunnel_id = ? AND user_id = ? AND status = 1`, ut.TunnelID, ut.UserID)
		s.flow.PauseForwardByIDs(forwards)
		expiredIDs = append(expiredIDs, ut.ID)
	}
	if len(expiredIDs) > 0 {
		q, args, err := sqlx.In(`UPDATE user_tunnel SET status = 0 WHERE id IN (?)`, expiredIDs)
		if err == nil {
			q = s.DB.Rebind(q)
			_, _ = s.DB.Exec(q, args...)
		}
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
	if len(users) == 0 {
		return
	}

	// 各用户上一次累计值一次查回（原逐用户 SELECT）
	lastTotal := make(map[int64]int64, len(users))
	var lastRows []struct {
		UserID    int64 `db:"user_id"`
		TotalFlow int64 `db:"total_flow"`
	}
	if err := s.DB.Select(&lastRows, `
		SELECT user_id, total_flow FROM statistics_flow
		WHERE id IN (SELECT MAX(id) FROM statistics_flow GROUP BY user_id)`); err == nil {
		for _, r := range lastRows {
			lastTotal[r.UserID] = r.TotalFlow
		}
	}

	type statRow struct {
		userID, flow, total int64
	}
	rows := make([]statRow, 0, len(users))
	for _, u := range users {
		currentTotal := u.InFlow + u.OutFlow
		increment := currentTotal
		if last, ok := lastTotal[u.ID]; ok {
			increment = currentTotal - last
			if increment < 0 {
				increment = currentTotal
			}
		}
		rows = append(rows, statRow{userID: u.ID, flow: increment, total: currentTotal})
	}
	// 多行单语句批量插入（原逐行 INSERT；每行 5 变量，分片 150 → 750 < 999 上限）
	for start := 0; start < len(rows); start += 150 {
		end := start + 150
		if end > len(rows) {
			end = len(rows)
		}
		var sb strings.Builder
		args := make([]any, 0, (end-start)*5)
		sb.WriteString(`INSERT INTO statistics_flow (user_id, flow, total_flow, time, created_time) VALUES `)
		for i, r := range rows[start:end] {
			if i > 0 {
				sb.WriteString(`,`)
			}
			sb.WriteString(`(?,?,?,?,?)`)
			args = append(args, r.userID, r.flow, r.total, hourString, ts)
		}
		if _, err := s.DB.Exec(sb.String(), args...); err != nil {
			slog.Warn("写入统计流量失败", "rows", end-start, "err", err)
		}
	}
}
