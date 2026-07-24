package handler

import (
	"github.com/Fearless743/flux-panel/go-backend/internal/middleware"
	"github.com/Fearless743/flux-panel/go-backend/internal/response"
	"github.com/Fearless743/flux-panel/go-backend/internal/service"
	"github.com/gin-gonic/gin"
)

func (a *App) forwardService() *service.ForwardService {
	return service.NewForwardService(a.DB, a.Hub)
}

func (a *App) currentUser(c *gin.Context) service.CurrentUser {
	return service.CurrentUser{
		UserID:   middleware.GetUserID(c),
		RoleID:   middleware.GetRoleID(c),
		UserName: middleware.GetName(c),
	}
}

func (a *App) ForwardCreate(c *gin.Context) {
	var req service.ForwardCreateReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if err := a.forwardService().Create(a.currentUser(c), req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) ForwardList(c *gin.Context) {
	list, err := a.forwardService().List(a.currentUser(c))
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, list)
}

func (a *App) ForwardUpdate(c *gin.Context) {
	var req service.ForwardUpdateReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if err := a.forwardService().Update(a.currentUser(c), req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) ForwardDelete(c *gin.Context) {
	var params map[string]any
	if err := c.ShouldBindJSON(&params); err != nil {
		response.Err(c, "参数错误")
		return
	}
	id, ok := asInt64(params["id"])
	if !ok || id == 0 {
		response.Err(c, "转发ID不能为空")
		return
	}
	if err := a.forwardService().Delete(a.currentUser(c), id); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) ForwardForceDelete(c *gin.Context) {
	var params map[string]any
	if err := c.ShouldBindJSON(&params); err != nil {
		response.Err(c, "参数错误")
		return
	}
	id, ok := asInt64(params["id"])
	if !ok || id == 0 {
		response.Err(c, "转发ID不能为空")
		return
	}
	if err := a.forwardService().ForceDelete(a.currentUser(c), id); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) ForwardPause(c *gin.Context) {
	var params map[string]any
	if err := c.ShouldBindJSON(&params); err != nil {
		response.Err(c, "参数错误")
		return
	}
	id, ok := asInt64(params["id"])
	if !ok || id == 0 {
		response.Err(c, "转发ID不能为空")
		return
	}
	if err := a.forwardService().Pause(a.currentUser(c), id); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) ForwardResume(c *gin.Context) {
	var params map[string]any
	if err := c.ShouldBindJSON(&params); err != nil {
		response.Err(c, "参数错误")
		return
	}
	id, ok := asInt64(params["id"])
	if !ok || id == 0 {
		response.Err(c, "转发ID不能为空")
		return
	}
	if err := a.forwardService().Resume(a.currentUser(c), id); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) ForwardDiagnose(c *gin.Context) {
	var params map[string]any
	if err := c.ShouldBindJSON(&params); err != nil {
		response.Err(c, "参数错误")
		return
	}
	id, ok := asInt64(params["forwardId"])
	if !ok || id == 0 {
		// 兼容 id 字段
		id, ok = asInt64(params["id"])
	}
	if !ok || id == 0 {
		response.Err(c, "转发ID不能为空")
		return
	}
	report, err := a.forwardService().Diagnose(a.currentUser(c), id)
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, report)
}

func (a *App) ForwardUpdateOrder(c *gin.Context) {
	var req service.UpdateOrderReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if len(req.Forwards) == 0 {
		// 兼容 {forwards: [...]} 之外的空情况
		response.Err(c, "缺少forwards参数")
		return
	}
	if err := a.forwardService().UpdateOrder(a.currentUser(c), req.Forwards); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) ForwardBatchDelete(c *gin.Context) {
	var req service.BatchForwardReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	msg, ok := a.forwardService().BatchDelete(a.currentUser(c), req.IDs)
	if !ok {
		response.Err(c, msg)
		return
	}
	response.OKMsg(c, msg, nil)
}

func (a *App) ForwardBatchPause(c *gin.Context) {
	var req service.BatchForwardReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	msg, ok := a.forwardService().BatchPause(a.currentUser(c), req.IDs)
	if !ok {
		response.Err(c, msg)
		return
	}
	response.OKMsg(c, msg, nil)
}

func (a *App) ForwardBatchResume(c *gin.Context) {
	var req service.BatchForwardReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	msg, ok := a.forwardService().BatchResume(a.currentUser(c), req.IDs)
	if !ok {
		response.Err(c, msg)
		return
	}
	response.OKMsg(c, msg, nil)
}

func (a *App) ForwardBatchChangeTunnel(c *gin.Context) {
	var req service.BatchForwardReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	tunnelID := 0
	if req.TunnelID != nil {
		tunnelID = *req.TunnelID
	}
	msg, ok := a.forwardService().BatchChangeTunnel(a.currentUser(c), req.IDs, tunnelID)
	if !ok {
		response.Err(c, msg)
		return
	}
	response.OKMsg(c, msg, nil)
}
