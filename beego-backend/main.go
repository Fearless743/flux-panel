package main

import (
	"log"

	"beego-backend/common"
	"beego-backend/middleware"
	_ "beego-backend/routers"

	"github.com/beego/beego/v2/server/web"
)

func main() {
	// 初始化数据库封装
	err := common.InitDatabase()
	if err != nil {
		log.Fatalf("❌ 数据库初始化失败: %v", err)
	}

	// 中间件
	web.InsertFilter("/*", web.BeforeRouter, middleware.BaseAuth)

	// 启动服务器
	web.Run()
}
