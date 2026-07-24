package service

import (
	"fmt"

	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/repo"
	"github.com/Fearless743/flux-panel/go-backend/internal/ws"
	"github.com/jmoiron/sqlx"
)

// UserTunnelService 对齐 Java UserTunnelServiceImpl
// 注意：Java updateUserTunnel 成功后恒返回 err 是 bug，Go 返回 nil 表示成功
type UserTunnelService struct {
	DB   *sqlx.DB
	Hub  *ws.Hub
	Repo *repo.UserTunnelRepo
}

func NewUserTunnelService(db *sqlx.DB, hub *ws.Hub) *UserTunnelService {
	return &UserTunnelService{
		DB:   db,
		Hub:  hub,
		Repo: repo.NewUserTunnelRepo(db),
	}
}

type UserTunnelAssignReq struct {
	UserID        int   `json:"userId"`
	TunnelID      int   `json:"tunnelId"`
	Flow          int64 `json:"flow"`
	Num           int   `json:"num"`
	FlowResetTime int64 `json:"flowResetTime"`
	ExpTime       int64 `json:"expTime"`
	SpeedID       *int  `json:"speedId"`
}

type UserTunnelListReq struct {
	UserID int `json:"userId"`
}

type UserTunnelUpdateReq struct {
	ID            int   `json:"id"`
	Flow          int64 `json:"flow"`
	Num           int   `json:"num"`
	FlowResetTime int64 `json:"flowResetTime"`
	ExpTime       int64 `json:"expTime"`
	Status        int   `json:"status"`
	SpeedID       *int  `json:"speedId"`
}

func (s *UserTunnelService) Assign(req UserTunnelAssignReq) error {
	if req.UserID == 0 {
		return fmt.Errorf("用户ID不能为空")
	}
	if req.TunnelID == 0 {
		return fmt.Errorf("隧道ID不能为空")
	}
	if req.Flow < 0 {
		return fmt.Errorf("流量限制不能小于0")
	}
	if req.Num < 0 {
		return fmt.Errorf("转发数量不能小于0")
	}
	cnt, err := s.Repo.CountByUserAndTunnel(req.UserID, req.TunnelID)
	if err != nil {
		return err
	}
	if cnt > 0 {
		return fmt.Errorf("该用户已拥有此隧道权限")
	}
	ut := &model.UserTunnel{
		UserID:        req.UserID,
		TunnelID:      req.TunnelID,
		SpeedID:       req.SpeedID,
		Num:           req.Num,
		Flow:          req.Flow,
		FlowResetTime: req.FlowResetTime,
		ExpTime:       req.ExpTime,
		Status:        1,
	}
	return s.Repo.Insert(ut)
}

func (s *UserTunnelService) List(userID int) ([]repo.UserTunnelDetail, error) {
	if userID == 0 {
		return nil, fmt.Errorf("用户ID不能为空")
	}
	list, err := s.Repo.ListDetailsByUserID(userID)
	if err != nil {
		return nil, err
	}
	if list == nil {
		list = []repo.UserTunnelDetail{}
	}
	return list, nil
}

func (s *UserTunnelService) Remove(id int) error {
	if id == 0 {
		return fmt.Errorf("ID不能为空")
	}
	ut, err := s.Repo.GetByID(id)
	if err != nil {
		return err
	}
	if ut == nil {
		return fmt.Errorf("未找到对应的用户隧道权限记录")
	}

	// 删除该用户在此隧道下的所有转发
	fwRepo := repo.NewForwardRepo(s.DB)
	forwards, err := fwRepo.ListByUserAndTunnel(ut.UserID, ut.TunnelID)
	if err != nil {
		return err
	}
	fwSvc := NewForwardService(s.DB, s.Hub)
	admin := CurrentUser{UserID: 0, RoleID: 0}
	for _, fw := range forwards {
		if err := fwSvc.Delete(admin, fw.ID); err != nil {
			// 尽力清理
			portRepo := repo.NewForwardPortRepo(s.DB)
			_ = portRepo.DeleteByForwardID(fw.ID)
			_ = fwRepo.Delete(fw.ID)
		}
	}
	return s.Repo.Delete(id)
}

func (s *UserTunnelService) Update(req UserTunnelUpdateReq) error {
	if req.ID == 0 {
		return fmt.Errorf("用户隧道权限ID不能为空")
	}
	if req.Flow < 0 {
		return fmt.Errorf("流量限制不能小于0")
	}
	if req.Num < 0 {
		return fmt.Errorf("转发数量不能小于0")
	}
	ut, err := s.Repo.GetByID(req.ID)
	if err != nil {
		return err
	}
	if ut == nil {
		return fmt.Errorf("隧道不存在")
	}

	speedChanged := hasSpeedChanged(ut.SpeedID, req.SpeedID)
	ut.Flow = req.Flow
	ut.Num = req.Num
	ut.FlowResetTime = req.FlowResetTime
	ut.ExpTime = req.ExpTime
	ut.Status = req.Status
	ut.SpeedID = req.SpeedID

	if err := s.Repo.Update(ut); err != nil {
		return err
	}

	// speed 变化 → 重建 forward 服务（对齐 Java updateForward）
	if speedChanged {
		fwRepo := repo.NewForwardRepo(s.DB)
		forwards, err := fwRepo.ListByUserAndTunnel(ut.UserID, ut.TunnelID)
		if err != nil {
			return err
		}
		fwSvc := NewForwardService(s.DB, s.Hub)
		admin := CurrentUser{UserID: 0, RoleID: 0}
		for _, fw := range forwards {
			upd := ForwardUpdateReq{
				ID:         fw.ID,
				UserID:     fw.UserID,
				Name:       fw.Name,
				RemoteAddr: fw.RemoteAddr,
				Strategy:   fw.Strategy,
			}
			// 不传 TunnelID → 走同隧道更新路径（会重建 limiter）
			if err := fwSvc.Update(admin, upd); err != nil {
				// 不阻断主更新；记录但继续
				_ = err
			}
		}
	}
	// Java 这里恒返回 err，Go 修 bug：成功返回 nil
	return nil
}

func hasSpeedChanged(oldID, newID *int) bool {
	if oldID == nil && newID == nil {
		return false
	}
	if oldID == nil || newID == nil {
		return true
	}
	return *oldID != *newID
}
