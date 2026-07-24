package response

import (
	"net/http"
	"time"

	"github.com/gin-gonic/gin"
)

// R 对齐 Java com.admin.common.lang.R
type R struct {
	Code int         `json:"code"`
	Msg  string      `json:"msg"`
	Ts   int64       `json:"ts"`
	Data interface{} `json:"data"`
}

func OK(c *gin.Context, data interface{}) {
	c.JSON(http.StatusOK, R{
		Code: 0,
		Msg:  "操作成功",
		Ts:   time.Now().UnixMilli(),
		Data: data,
	})
}

func OKMsg(c *gin.Context, msg string, data interface{}) {
	c.JSON(http.StatusOK, R{
		Code: 0,
		Msg:  msg,
		Ts:   time.Now().UnixMilli(),
		Data: data,
	})
}

func Err(c *gin.Context, msg string) {
	c.JSON(http.StatusOK, R{
		Code: -1,
		Msg:  msg,
		Ts:   time.Now().UnixMilli(),
		Data: nil,
	})
}

func ErrCode(c *gin.Context, code int, msg string) {
	c.JSON(http.StatusOK, R{
		Code: code,
		Msg:  msg,
		Ts:   time.Now().UnixMilli(),
		Data: nil,
	})
}

// Unauthorized 对齐前端 token 失效判断
func Unauthorized(c *gin.Context, msg string) {
	if msg == "" {
		msg = "未登录或token已过期"
	}
	c.JSON(http.StatusOK, R{
		Code: 401,
		Msg:  msg,
		Ts:   time.Now().UnixMilli(),
		Data: nil,
	})
	c.Abort()
}

func Forbidden(c *gin.Context, msg string) {
	if msg == "" {
		msg = "无权限访问"
	}
	c.JSON(http.StatusOK, R{
		Code: 403,
		Msg:  msg,
		Ts:   time.Now().UnixMilli(),
		Data: nil,
	})
	c.Abort()
}
