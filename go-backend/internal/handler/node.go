package handler

import (
	"github.com/Fearless743/flux-panel/go-backend/internal/response"
	"github.com/Fearless743/flux-panel/go-backend/internal/service"
	"github.com/gin-gonic/gin"
)

func (a *App) nodeService() *service.NodeService {
	return service.NewNodeService(a.DB, a.Hub)
}

func (a *App) NodeCreate(c *gin.Context) {
	var req service.NodeCreateReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if err := a.nodeService().Create(req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) NodeList(c *gin.Context) {
	list, err := a.nodeService().List()
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, list)
}

func (a *App) NodeUpdate(c *gin.Context) {
	var req service.NodeUpdateReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	if err := a.nodeService().Update(req); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) NodeDelete(c *gin.Context) {
	var params map[string]any
	if err := c.ShouldBindJSON(&params); err != nil {
		response.Err(c, "参数错误")
		return
	}
	id, ok := asInt64(params["id"])
	if !ok || id == 0 {
		response.Err(c, "节点ID不能为空")
		return
	}
	if err := a.nodeService().Delete(id); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}

func (a *App) NodeInstall(c *gin.Context) {
	var params map[string]any
	if err := c.ShouldBindJSON(&params); err != nil {
		response.Err(c, "参数错误")
		return
	}
	id, ok := asInt64(params["id"])
	if !ok || id == 0 {
		response.Err(c, "节点ID不能为空")
		return
	}
	cmd, err := a.nodeService().InstallCommand(id)
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, cmd)
}

// NodeUpgrade POST /node/upgrade 远程升级在线节点二进制
func (a *App) NodeUpgrade(c *gin.Context) {
	var req service.NodeUpgradeReq
	if err := c.ShouldBindJSON(&req); err != nil {
		response.Err(c, "参数错误")
		return
	}
	msg, err := a.nodeService().Upgrade(req)
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, msg)
}

func asInt64(v any) (int64, bool) {
	switch t := v.(type) {
	case float64:
		return int64(t), true
	case int64:
		return t, true
	case int:
		return int64(t), true
	case string:
		var n int64
		for _, ch := range t {
			if ch < '0' || ch > '9' {
				return 0, false
			}
			n = n*10 + int64(ch-'0')
		}
		return n, true
	default:
		return 0, false
	}
}
