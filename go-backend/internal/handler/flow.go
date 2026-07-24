package handler

import (
	"encoding/json"
	"io"
	"log/slog"
	"net/http"

	"github.com/Fearless743/flux-panel/go-backend/internal/service"
	"github.com/gin-gonic/gin"
)

func (a *App) flowService() *service.FlowService {
	return service.NewFlowService(a.DB, a.Hub)
}

// FlowTest 返回纯文本 "test"
func (a *App) FlowTest(c *gin.Context) {
	c.String(http.StatusOK, "test")
}

// FlowUpload ?secret= 解密 JSON 数组并计费
func (a *App) FlowUpload(c *gin.Context) {
	secret := c.Query("secret")
	if secret == "" {
		secret = c.PostForm("secret")
	}
	fs := a.flowService()
	if !fs.IsValidNode(secret) {
		c.String(http.StatusOK, "ok")
		return
	}

	raw, err := io.ReadAll(c.Request.Body)
	if err != nil {
		c.String(http.StatusOK, "ok")
		return
	}
	plain, err := service.DecryptIfNeeded(raw, secret)
	if err != nil {
		slog.Warn("flow upload decrypt", "err", err)
		c.String(http.StatusOK, "ok")
		return
	}

	var items []service.FlowDto
	if err := json.Unmarshal(plain, &items); err != nil {
		slog.Warn("flow upload parse", "err", err)
		c.String(http.StatusOK, "ok")
		return
	}
	slog.Info("节点上报流量数据", "count", len(items))
	fs.Upload(items)
	c.String(http.StatusOK, "ok")
}

// FlowConfig ?secret= 解密配置并异步 CleanNodeConfigs
func (a *App) FlowConfig(c *gin.Context) {
	secret := c.Query("secret")
	if secret == "" {
		secret = c.PostForm("secret")
	}
	fs := a.flowService()
	node, err := fs.FindNodeBySecret(secret)
	if err != nil || node == nil {
		c.String(http.StatusOK, "ok")
		return
	}

	raw, err := io.ReadAll(c.Request.Body)
	if err != nil {
		c.String(http.StatusOK, "ok")
		return
	}
	plain, err := service.DecryptIfNeeded(raw, secret)
	if err != nil {
		slog.Warn("flow config decrypt", "err", err)
		c.String(http.StatusOK, "ok")
		return
	}

	var cfg service.GostConfigDto
	if err := json.Unmarshal(plain, &cfg); err != nil {
		slog.Warn("flow config parse", "err", err, "node", node.ID)
		c.String(http.StatusOK, "ok")
		return
	}

	go fs.CleanNodeConfigs(node.ID, cfg)
	slog.Info("节点配置数据接收成功", "node", node.ID)
	c.String(http.StatusOK, "ok")
}
