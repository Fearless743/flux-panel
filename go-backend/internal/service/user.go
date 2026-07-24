package service

import (
	"errors"
	"strings"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/auth"
	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/repo"
	"github.com/jmoiron/sqlx"
)

type UserService struct {
	repo    *repo.UserRepo
	cfg     *ConfigService
	captcha *CaptchaService
	jwt     *auth.JWT
}

func NewUserService(db *sqlx.DB, jwt *auth.JWT) *UserService {
	return &UserService{
		repo:    repo.NewUserRepo(db),
		cfg:     NewConfigService(db),
		captcha: NewCaptchaService(db),
		jwt:     jwt,
	}
}

type LoginReq struct {
	Username  string `json:"username"`
	Password  string `json:"password"`
	CaptchaID string `json:"captchaId"`
}

type LoginResp struct {
	Token                 string `json:"token"`
	Name                  string `json:"name"`
	RoleID                int    `json:"role_id"`
	RequirePasswordChange bool   `json:"requirePasswordChange"`
}

func (s *UserService) Login(req LoginReq) (*LoginResp, error) {
	if strings.TrimSpace(req.Username) == "" || strings.TrimSpace(req.Password) == "" {
		return nil, errors.New("账号或密码错误")
	}
	if s.cfg.IsCaptchaEnabled() {
		if strings.TrimSpace(req.CaptchaID) == "" || !s.captcha.Secondary(req.CaptchaID) {
			return nil, errors.New("验证码校验失败")
		}
	}
	u, err := s.repo.FindByUsername(req.Username)
	if err != nil {
		return nil, err
	}
	if u == nil || !auth.CheckPassword(req.Password, u.Pwd) {
		return nil, errors.New("账号或密码错误")
	}
	if u.Status == 0 {
		return nil, errors.New("账号被停用")
	}
	token, err := s.jwt.Generate(u.ID, u.User, u.RoleID)
	if err != nil {
		return nil, err
	}
	requireChange := req.Username == "admin_user" || req.Password == "admin_user"
	return &LoginResp{
		Token:                 token,
		Name:                  u.User,
		RoleID:                u.RoleID,
		RequirePasswordChange: requireChange,
	}, nil
}

type CreateUserReq struct {
	User          string `json:"user"`
	Pwd           string `json:"pwd"`
	Flow          int64  `json:"flow"`
	Num           int    `json:"num"`
	ExpTime       int64  `json:"expTime"`
	FlowResetTime int64  `json:"flowResetTime"`
	Status        *int   `json:"status"`
}

func (s *UserService) Create(req CreateUserReq) error {
	if strings.TrimSpace(req.User) == "" {
		return errors.New("用户名不能为空")
	}
	if strings.TrimSpace(req.Pwd) == "" {
		return errors.New("密码不能为空")
	}
	n, err := s.repo.CountByUsername(req.User, 0)
	if err != nil {
		return err
	}
	if n > 0 {
		return errors.New("用户名已存在")
	}
	now := time.Now().UnixMilli()
	status := 1
	if req.Status != nil {
		status = *req.Status
	}
	u := &model.User{
		User:          req.User,
		Pwd:           auth.MD5(req.Pwd),
		RoleID:        1,
		ExpTime:       req.ExpTime,
		Flow:          req.Flow,
		FlowResetTime: req.FlowResetTime,
		Num:           req.Num,
		CreatedTime:   now,
		UpdatedTime:   &now,
		Status:        status,
	}
	return s.repo.Create(u)
}

func (s *UserService) List() ([]model.User, error) {
	return s.repo.ListNonAdmin()
}

type UpdateUserReq struct {
	ID            int64  `json:"id"`
	User          string `json:"user"`
	Pwd           string `json:"pwd"`
	Flow          int64  `json:"flow"`
	Num           int    `json:"num"`
	ExpTime       int64  `json:"expTime"`
	FlowResetTime int64  `json:"flowResetTime"`
	Status        *int   `json:"status"`
}

func (s *UserService) Update(req UpdateUserReq) error {
	u, err := s.repo.FindByID(req.ID)
	if err != nil {
		return err
	}
	if u == nil {
		return errors.New("用户不存在")
	}
	if u.RoleID == 0 {
		return errors.New("请不要作死")
	}
	n, err := s.repo.CountByUsername(req.User, req.ID)
	if err != nil {
		return err
	}
	if n > 0 {
		return errors.New("用户名已存在")
	}
	now := time.Now().UnixMilli()
	u.User = req.User
	u.Flow = req.Flow
	u.Num = req.Num
	u.ExpTime = req.ExpTime
	u.FlowResetTime = req.FlowResetTime
	u.UpdatedTime = &now
	if req.Status != nil {
		u.Status = *req.Status
	}
	updatePwd := strings.TrimSpace(req.Pwd) != ""
	if updatePwd {
		u.Pwd = auth.MD5(req.Pwd)
	}
	return s.repo.Update(u, updatePwd)
}

func (s *UserService) Delete(id int64) error {
	u, err := s.repo.FindByID(id)
	if err != nil {
		return err
	}
	if u == nil {
		return errors.New("用户不存在")
	}
	if u.RoleID == 0 {
		return errors.New("请不要作死")
	}
	// 先删 forward/ports（无完整 forward 服务时直接 DB 清理）
	if err := s.repo.DeleteForwardsByUser(id); err != nil {
		return err
	}
	if err := s.repo.DeleteUserTunnels(id); err != nil {
		return err
	}
	if err := s.repo.DeleteStatisticsFlow(id); err != nil {
		return err
	}
	return s.repo.Delete(id)
}

type UserInfoDto struct {
	ID            int64  `json:"id"`
	User          string `json:"user"`
	Status        int    `json:"status"`
	Flow          int64  `json:"flow"`
	InFlow        int64  `json:"inFlow"`
	OutFlow       int64  `json:"outFlow"`
	Num           int    `json:"num"`
	ExpTime       int64  `json:"expTime"`
	FlowResetTime int64  `json:"flowResetTime"`
	CreatedTime   int64  `json:"createdTime"`
	UpdatedTime   *int64 `json:"updatedTime"`
}

type PackageDto struct {
	UserInfo          UserInfoDto                      `json:"userInfo"`
	TunnelPermissions []repo.PackageUserTunnelDetail   `json:"tunnelPermissions"`
	Forwards          []repo.UserForwardDetail         `json:"forwards"`
	StatisticsFlows   []model.StatisticsFlow           `json:"statisticsFlows"`
}

func (s *UserService) Package(userID int64) (*PackageDto, error) {
	u, err := s.repo.FindByID(userID)
	if err != nil {
		return nil, err
	}
	if u == nil {
		return nil, errors.New("用户不存在")
	}
	tunnels, err := s.repo.GetUserTunnelDetails(userID)
	if err != nil {
		return nil, err
	}
	forwards, err := s.repo.GetUserForwardDetails(userID)
	if err != nil {
		return nil, err
	}
	if err := s.repo.FillForwardInIPAndPort(forwards); err != nil {
		return nil, err
	}
	stats, err := s.repo.GetLast24HoursFlow(userID)
	if err != nil {
		return nil, err
	}
	return &PackageDto{
		UserInfo: UserInfoDto{
			ID:            u.ID,
			User:          u.User,
			Status:        u.Status,
			Flow:          u.Flow,
			InFlow:        u.InFlow,
			OutFlow:       u.OutFlow,
			Num:           u.Num,
			ExpTime:       u.ExpTime,
			FlowResetTime: u.FlowResetTime,
			CreatedTime:   u.CreatedTime,
			UpdatedTime:   u.UpdatedTime,
		},
		TunnelPermissions: tunnels,
		Forwards:          forwards,
		StatisticsFlows:   stats,
	}, nil
}

type ChangePasswordReq struct {
	NewUsername     string `json:"newUsername"`
	CurrentPassword string `json:"currentPassword"`
	NewPassword     string `json:"newPassword"`
	ConfirmPassword string `json:"confirmPassword"`
}

func (s *UserService) UpdatePassword(userID int64, req ChangePasswordReq) error {
	u, err := s.repo.FindByID(userID)
	if err != nil {
		return err
	}
	if u == nil {
		return errors.New("用户不存在")
	}
	if req.NewPassword != req.ConfirmPassword {
		return errors.New("新密码和确认密码不匹配")
	}
	if !auth.CheckPassword(req.CurrentPassword, u.Pwd) {
		return errors.New("当前密码错误")
	}
	if u.User != req.NewUsername {
		n, err := s.repo.CountByUsername(req.NewUsername, u.ID)
		if err != nil {
			return err
		}
		if n > 0 {
			return errors.New("用户名已存在")
		}
	}
	return s.repo.UpdatePasswordAndUsername(u.ID, req.NewUsername, auth.MD5(req.NewPassword))
}

type ResetFlowReq struct {
	ID   int64 `json:"id"`
	Type int   `json:"type"`
}

func (s *UserService) Reset(req ResetFlowReq) error {
	if req.Type == 1 {
		u, err := s.repo.FindByID(req.ID)
		if err != nil {
			return err
		}
		if u == nil {
			return errors.New("用户不存在")
		}
		return s.repo.ResetUserFlow(req.ID)
	}
	ut, err := s.repo.FindUserTunnelByID(req.ID)
	if err != nil {
		return err
	}
	if ut == nil {
		return errors.New("隧道不存在")
	}
	return s.repo.ResetUserTunnelFlow(req.ID)
}

// OpenAPISubStore 订阅信息
func (s *UserService) OpenAPISubStore(user, pwd, tunnel string) (string, error) {
	if strings.TrimSpace(user) == "" {
		return "", errors.New("用户不能为空")
	}
	if strings.TrimSpace(pwd) == "" {
		return "", errors.New("密码不能为空")
	}
	u, err := s.repo.FindByUsername(user)
	if err != nil {
		return "", err
	}
	if u == nil || !auth.CheckPassword(pwd, u.Pwd) {
		return "", errors.New("鉴权失败")
	}
	const giga = 1024 * 1024 * 1024
	if tunnel == "" || tunnel == "-1" {
		// 注意交叉：upload 头放 inFlow，download 放 outFlow（与 Java 一致）
		return buildSubscriptionHeader(u.OutFlow, u.InFlow, u.Flow*giga, u.ExpTime/1000), nil
	}
	// tunnel 为 user_tunnel.id
	var tid int64
	for _, ch := range tunnel {
		if ch < '0' || ch > '9' {
			return "", errors.New("隧道不存在")
		}
		tid = tid*10 + int64(ch-'0')
	}
	ut, err := s.repo.FindUserTunnelByID(tid)
	if err != nil {
		return "", err
	}
	if ut == nil || int64(ut.UserID) != u.ID {
		return "", errors.New("隧道不存在")
	}
	return buildSubscriptionHeader(ut.OutFlow, ut.InFlow, ut.Flow*giga, ut.ExpTime/1000), nil
}

// buildSubscriptionHeader(upload, download, total, expire)
// Java: format("upload=%d; download=%d; total=%d; expire=%d", download, upload, total, expire)
// 即 upload 参数实际写入 download 位置，download 参数写入 upload 位置
func buildSubscriptionHeader(upload, download, total, expire int64) string {
	return strings.Join([]string{
		"upload=" + itoa(download),
		" download=" + itoa(upload),
		" total=" + itoa(total),
		" expire=" + itoa(expire),
	}, ";")
}

func itoa(v int64) string {
	if v == 0 {
		return "0"
	}
	neg := v < 0
	if neg {
		v = -v
	}
	var b [20]byte
	i := len(b)
	for v > 0 {
		i--
		b[i] = byte('0' + v%10)
		v /= 10
	}
	if neg {
		i--
		b[i] = '-'
	}
	return string(b[i:])
}
