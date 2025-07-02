package routers

import (
	"beego-backend/controllers"
	"github.com/beego/beego/v2/server/web"
	"github.com/beego/beego/v2/server/web/context"
)

func init() {
	web.InsertFilter("*", web.BeforeRouter, func(ctx *context.Context) {
		ctx.Output.Header("Access-Control-Allow-Origin", "*")
		ctx.Output.Header("Access-Control-Allow-Methods", "POST, GET, OPTIONS, PUT, DELETE")
		ctx.Output.Header("Access-Control-Allow-Headers", "Origin, X-Requested-With, Content-Type, Accept, token")
		if ctx.Input.Method() == "OPTIONS" {
			ctx.Output.SetStatus(200)
			ctx.ResponseWriter.WriteHeader(200)
			return
		}
	})

	ns := web.NewNamespace("/api/v1/",
		web.NSNamespace("/login",
			web.NSInclude(
				&controllers.LoginController{},
			),
		),
	)
	web.AddNamespace(ns)
}
