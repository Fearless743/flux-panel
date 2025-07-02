package middleware

import (
	"github.com/beego/beego/v2/server/web/context"
)

var BaseAuth = func(ctx *context.Context) {
	//secret := ctx.Input.Query("secret")
	//if secret == "" {
	//	resp := common.ResponseResult{
	//		Code:    -1,
	//		Success: false,
	//		Message: "数据库链接失败",
	//		Data:    nil,
	//	}
	//	ctx.Output.JSON(resp, false, false)
	//	ctx.Abort(403, "")
	//}

}
