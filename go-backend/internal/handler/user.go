package handler

import (
	"github.com/Fearless743/flux-panel/go-backend/internal/middleware"
	"github.com/Fearless743/flux-panel/go-backend/internal/response"
	"github.com/Fearless743/flux-panel/go-backend/internal/service"
	"github.com/gin-gonic/gin"
)

func (a *App) userSvc() *service.UserService {
	return service.NewUserService(a.DB, a.JWT)
}

func (a *App) UserLogin(c *gin.Context) {
	var req service.LoginReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	resp, err := a.userSvc().Login(req)
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, resp)
}

func (a *App) UserCreate(c *gin.Context) {
	var req service.CreateUserReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if err := a.userSvc().Create(req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) UserList(c *gin.Context) {
	list, err := a.userSvc().List()
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, list)
}

func (a *App) UserUpdate(c *gin.Context) {
	var req service.UpdateUserReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if err := a.userSvc().Update(req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) UserDelete(c *gin.Context) {
	var body struct {
		ID int64 `json:"id"`
	}
	if err := c.ShouldBindJSON(&body); err != nil || body.ID == 0 {
		response.Err(c, "参数错误")
		return
	}
	if err := a.userSvc().Delete(body.ID); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) UserPackage(c *gin.Context) {
	uid := middleware.GetUserID(c)
	pkg, err := a.userSvc().Package(uid)
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, pkg)
}

func (a *App) UserUpdatePassword(c *gin.Context) {
	var req service.ChangePasswordReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	uid := middleware.GetUserID(c)
	if err := a.userSvc().UpdatePassword(uid, req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) UserReset(c *gin.Context) {
	var req service.ResetFlowReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if err := a.userSvc().Reset(req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}
