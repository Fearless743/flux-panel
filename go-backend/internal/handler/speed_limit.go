package handler

import (
	"github.com/Fearless743/flux-panel/go-backend/internal/response"
	"github.com/Fearless743/flux-panel/go-backend/internal/service"
	"github.com/gin-gonic/gin"
)

// SpeedLimitCreate POST /api/v1/speed-limit/create
func (a *App) SpeedLimitCreate(c *gin.Context) {
	var req service.CreateSpeedLimitReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	svc := service.NewSpeedLimitService(a.DB, a.Hub)
	if err := svc.Create(req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

// SpeedLimitList POST /api/v1/speed-limit/list
func (a *App) SpeedLimitList(c *gin.Context) {
	svc := service.NewSpeedLimitService(a.DB, a.Hub)
	list, err := svc.List()
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, list)
}

// SpeedLimitUpdate POST /api/v1/speed-limit/update
func (a *App) SpeedLimitUpdate(c *gin.Context) {
	var req service.UpdateSpeedLimitReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	svc := service.NewSpeedLimitService(a.DB, a.Hub)
	if err := svc.Update(req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

// SpeedLimitDelete POST /api/v1/speed-limit/delete
func (a *App) SpeedLimitDelete(c *gin.Context) {
	var params struct {
		ID int64 `json:"id"`
	}
	if err := c.ShouldBindJSON(&params); err != nil {
		response.Err(c, "参数错误")
		return
	}
	svc := service.NewSpeedLimitService(a.DB, a.Hub)
	if err := svc.Delete(params.ID); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

// SpeedLimitTunnels POST /api/v1/speed-limit/tunnels
func (a *App) SpeedLimitTunnels(c *gin.Context) {
	svc := service.NewSpeedLimitService(a.DB, a.Hub)
	list, err := svc.ListTunnels()
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, list)
}
