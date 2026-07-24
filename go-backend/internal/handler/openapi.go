package handler

import (
	"github.com/Fearless743/flux-panel/go-backend/internal/response"
	"github.com/Fearless743/flux-panel/go-backend/internal/service"
	"github.com/gin-gonic/gin"
)

func (a *App) OpenAPISubStore(c *gin.Context) {
	user := c.Query("user")
	pwd := c.Query("pwd")
	tunnel := c.DefaultQuery("tunnel", "-1")

	header, err := service.NewUserService(a.DB, a.JWT).OpenAPISubStore(user, pwd, tunnel)
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	c.Header("subscription-userinfo", header)
	// Java 成功 body 为 header 字符串
	c.String(200, header)
}
