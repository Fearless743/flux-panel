package handler

import (
	"github.com/Fearless743/flux-panel/go-backend/internal/middleware"
	"github.com/Fearless743/flux-panel/go-backend/internal/response"
	"github.com/Fearless743/flux-panel/go-backend/internal/service"
	"github.com/gin-gonic/gin"
)

func (a *App) tunnelSvc() *service.TunnelService {
	return service.NewTunnelService(a.DB, a.Hub)
}

func (a *App) userTunnelSvc() *service.UserTunnelService {
	return service.NewUserTunnelService(a.DB, a.Hub)
}

// TunnelCreate POST /api/v1/tunnel/create
func (a *App) TunnelCreate(c *gin.Context) {
	var req service.TunnelCreateReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if err := a.tunnelSvc().Create(req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

// TunnelList POST /api/v1/tunnel/list
func (a *App) TunnelList(c *gin.Context) {
	list, err := a.tunnelSvc().List()
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, list)
}

// TunnelUpdate POST /api/v1/tunnel/update
func (a *App) TunnelUpdate(c *gin.Context) {
	var req service.TunnelUpdateReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if err := a.tunnelSvc().Update(req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

// TunnelDelete POST /api/v1/tunnel/delete
func (a *App) TunnelDelete(c *gin.Context) {
	var body struct {
		ID int64 `json:"id"`
	}
	if err := c.ShouldBindJSON(&body); err != nil || body.ID == 0 {
		response.Err(c, "参数错误")
		return
	}
	if err := a.tunnelSvc().Delete(body.ID); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

// TunnelDiagnose POST /api/v1/tunnel/diagnose
func (a *App) TunnelDiagnose(c *gin.Context) {
	var body struct {
		TunnelID int64 `json:"tunnelId"`
	}
	if err := c.ShouldBindJSON(&body); err != nil || body.TunnelID == 0 {
		response.Err(c, "参数错误")
		return
	}
	report, err := a.tunnelSvc().Diagnose(body.TunnelID)
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, report)
}

// UserTunnelAssign POST /api/v1/tunnel/user/assign
func (a *App) UserTunnelAssign(c *gin.Context) {
	var req service.UserTunnelAssignReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if err := a.userTunnelSvc().Assign(req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

// UserTunnelList POST /api/v1/tunnel/user/list
func (a *App) UserTunnelList(c *gin.Context) {
	var req service.UserTunnelListReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	list, err := a.userTunnelSvc().List(req.UserID)
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, list)
}

// UserTunnelRemove POST /api/v1/tunnel/user/remove
func (a *App) UserTunnelRemove(c *gin.Context) {
	var body struct {
		ID int `json:"id"`
	}
	if err := c.ShouldBindJSON(&body); err != nil || body.ID == 0 {
		response.Err(c, "参数错误")
		return
	}
	if err := a.userTunnelSvc().Remove(body.ID); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

// UserTunnelUpdate POST /api/v1/tunnel/user/update
// 注意：成功返回 ok（Java 恒 err 是 bug）
func (a *App) UserTunnelUpdate(c *gin.Context) {
	var req service.UserTunnelUpdateReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if err := a.userTunnelSvc().Update(req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

// UserTunnelMine POST /api/v1/tunnel/user/tunnel
func (a *App) UserTunnelMine(c *gin.Context) {
	uid := middleware.GetUserID(c)
	role := middleware.GetRoleID(c)
	list, err := a.tunnelSvc().Mine(uid, role)
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, list)
}
