package handler

import (
	"github.com/Fearless743/flux-panel/go-backend/internal/response"
	"github.com/Fearless743/flux-panel/go-backend/internal/service"
	"github.com/gin-gonic/gin"
)

func (a *App) configSvc() *service.ConfigService {
	return service.NewConfigService(a.DB)
}

func (a *App) ConfigGet(c *gin.Context) {
	var body struct {
		Name string `json:"name"`
	}
	if err := c.ShouldBindJSON(&body); err != nil {
		response.Err(c, "参数错误")
		return
	}
	cfg, err := a.configSvc().GetByName(body.Name)
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, cfg)
}

func (a *App) ConfigList(c *gin.Context) {
	m, err := a.configSvc().ListMap()
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, m)
}

func (a *App) ConfigUpdate(c *gin.Context) {
	var body map[string]string
	if err := c.ShouldBindJSON(&body); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if err := a.configSvc().UpdateMap(body); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) ConfigUpdateSingle(c *gin.Context) {
	var body struct {
		Name  string `json:"name"`
		Value string `json:"value"`
	}
	if err := c.ShouldBindJSON(&body); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if err := a.configSvc().UpdateSingle(body.Name, body.Value); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}
