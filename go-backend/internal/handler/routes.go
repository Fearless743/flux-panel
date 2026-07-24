package handler

import (
	"github.com/Fearless743/flux-panel/go-backend/internal/middleware"
	"github.com/gin-gonic/gin"
)

// RegisterRoutes 路径与 Java Controller 1:1
func RegisterRoutes(r *gin.Engine, app *App) {
	// 健康检查 / 节点上报（无 JWT）
	flow := r.Group("/flow")
	{
		flow.Any("/test", app.FlowTest)
		flow.POST("/upload", app.FlowUpload)
		flow.POST("/config", app.FlowConfig)
	}

	// WebSocket
	r.GET("/system-info", app.HandleWebSocket)

	api := r.Group("/api/v1")
	{
		// public
		api.POST("/user/login", app.UserLogin)
		api.POST("/captcha/check", app.CaptchaCheck)
		api.POST("/captcha/generate", app.CaptchaGenerate)
		api.POST("/captcha/verify", app.CaptchaVerify)
		api.POST("/config/get", app.ConfigGet)
		api.GET("/open_api/sub_store", app.OpenAPISubStore)

		// JWT
		authz := api.Group("")
		authz.Use(middleware.JWT(app.JWT))
		{
			// user
			authz.POST("/user/package", app.UserPackage)
			authz.POST("/user/updatePassword", app.UserUpdatePassword)

			admin := authz.Group("")
			admin.Use(middleware.Admin())
			{
				admin.POST("/user/create", app.UserCreate)
				admin.POST("/user/list", app.UserList)
				admin.POST("/user/update", app.UserUpdate)
				admin.POST("/user/delete", app.UserDelete)
				admin.POST("/user/reset", app.UserReset)

				admin.POST("/node/create", app.NodeCreate)
				admin.POST("/node/list", app.NodeList)
				admin.POST("/node/update", app.NodeUpdate)
				admin.POST("/node/delete", app.NodeDelete)
				admin.POST("/node/install", app.NodeInstall)

				admin.POST("/tunnel/create", app.TunnelCreate)
				admin.POST("/tunnel/list", app.TunnelList)
				admin.POST("/tunnel/update", app.TunnelUpdate)
				admin.POST("/tunnel/delete", app.TunnelDelete)
				admin.POST("/tunnel/diagnose", app.TunnelDiagnose)
				admin.POST("/tunnel/user/assign", app.UserTunnelAssign)
				admin.POST("/tunnel/user/list", app.UserTunnelList)
				admin.POST("/tunnel/user/remove", app.UserTunnelRemove)
				admin.POST("/tunnel/user/update", app.UserTunnelUpdate)

				admin.POST("/speed-limit/create", app.SpeedLimitCreate)
				admin.POST("/speed-limit/list", app.SpeedLimitList)
				admin.POST("/speed-limit/update", app.SpeedLimitUpdate)
				admin.POST("/speed-limit/delete", app.SpeedLimitDelete)
				admin.POST("/speed-limit/tunnels", app.SpeedLimitTunnels)

				admin.POST("/config/list", app.ConfigList)
				admin.POST("/config/update", app.ConfigUpdate)
				admin.POST("/config/update-single", app.ConfigUpdateSingle)
			}

			authz.POST("/tunnel/user/tunnel", app.UserTunnelMine)

			// forward（用户可操作自己的）
			authz.POST("/forward/create", app.ForwardCreate)
			authz.POST("/forward/list", app.ForwardList)
			authz.POST("/forward/update", app.ForwardUpdate)
			authz.POST("/forward/delete", app.ForwardDelete)
			authz.POST("/forward/force-delete", app.ForwardForceDelete)
			authz.POST("/forward/pause", app.ForwardPause)
			authz.POST("/forward/resume", app.ForwardResume)
			authz.POST("/forward/diagnose", app.ForwardDiagnose)
			authz.POST("/forward/update-order", app.ForwardUpdateOrder)
			authz.POST("/forward/batch-delete", app.ForwardBatchDelete)
			authz.POST("/forward/batch-pause", app.ForwardBatchPause)
			authz.POST("/forward/batch-resume", app.ForwardBatchResume)
			authz.POST("/forward/batch-change-tunnel", app.ForwardBatchChangeTunnel)
		}
	}
}
