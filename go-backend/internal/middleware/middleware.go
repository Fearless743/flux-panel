package middleware

import (
	"github.com/Fearless743/flux-panel/go-backend/internal/auth"
	"github.com/Fearless743/flux-panel/go-backend/internal/response"
	"github.com/gin-gonic/gin"
)

const (
	CtxUserID = "userId"
	CtxRoleID = "roleId"
	CtxName   = "userName"
	CtxToken  = "token"
)

func CORS() gin.HandlerFunc {
	return func(c *gin.Context) {
		c.Header("Access-Control-Allow-Origin", "*")
		c.Header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
		c.Header("Access-Control-Allow-Headers", "Origin, Content-Type, Accept, Authorization")
		c.Header("Access-Control-Expose-Headers", "Authorization")
		if c.Request.Method == "OPTIONS" {
			c.AbortWithStatus(204)
			return
		}
		c.Next()
	}
}

func JWT(j *auth.JWT) gin.HandlerFunc {
	return func(c *gin.Context) {
		token := c.GetHeader("Authorization")
		if token == "" {
			response.Unauthorized(c, "未登录或token已过期")
			return
		}
		claims, err := j.Parse(token)
		if err != nil {
			response.Unauthorized(c, "无效的token或token已过期")
			return
		}
		uid, err := j.UserID(token)
		if err != nil {
			response.Unauthorized(c, "无效的token或token已过期")
			return
		}
		c.Set(CtxUserID, uid)
		c.Set(CtxRoleID, claims.RoleID)
		c.Set(CtxName, claims.Name)
		c.Set(CtxToken, token)
		c.Next()
	}
}

// Admin role_id == 0
func Admin() gin.HandlerFunc {
	return func(c *gin.Context) {
		v, ok := c.Get(CtxRoleID)
		if !ok {
			response.Unauthorized(c, "无法获取用户权限信息")
			return
		}
		roleID, _ := v.(int)
		if roleID != 0 {
			response.Forbidden(c, "无权限访问")
			return
		}
		c.Next()
	}
}

func GetUserID(c *gin.Context) int64 {
	v, _ := c.Get(CtxUserID)
	id, _ := v.(int64)
	return id
}

func GetRoleID(c *gin.Context) int {
	v, _ := c.Get(CtxRoleID)
	id, _ := v.(int)
	return id
}

func GetName(c *gin.Context) string {
	v, _ := c.Get(CtxName)
	s, _ := v.(string)
	return s
}
