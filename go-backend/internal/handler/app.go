package handler

import (
	"github.com/Fearless743/flux-panel/go-backend/internal/auth"
	"github.com/Fearless743/flux-panel/go-backend/internal/config"
	"github.com/Fearless743/flux-panel/go-backend/internal/ws"
	"github.com/jmoiron/sqlx"
)

// App 共享依赖，各 handler/service 通过它访问
type App struct {
	DB     *sqlx.DB
	JWT    *auth.JWT
	Hub    *ws.Hub
	Config config.Config
}
