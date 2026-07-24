package main

import (
	"log/slog"
	"os"

	"github.com/Fearless743/flux-panel/go-backend/internal/auth"
	"github.com/Fearless743/flux-panel/go-backend/internal/config"
	"github.com/Fearless743/flux-panel/go-backend/internal/db"
	"github.com/Fearless743/flux-panel/go-backend/internal/handler"
	"github.com/Fearless743/flux-panel/go-backend/internal/middleware"
	"github.com/Fearless743/flux-panel/go-backend/internal/task"
	"github.com/Fearless743/flux-panel/go-backend/internal/ws"
	"github.com/gin-gonic/gin"
)

func main() {
	cfg := config.Load()
	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelInfo})))

	database, err := db.Open(cfg.DBPath)
	if err != nil {
		slog.Error("open db", "err", err)
		os.Exit(1)
	}
	defer database.Close()

	jwt := auth.NewJWT(cfg.JWTSecret)
	hub := ws.NewHub()

	app := &handler.App{
		DB:     database,
		JWT:    jwt,
		Hub:    hub,
		Config: cfg,
	}

	// 定时任务：流量重置 / 统计 / WAL checkpoint
	sched := task.New(database, hub)
	sched.Start()
	defer sched.Stop()

	r := gin.New()
	r.Use(gin.Recovery(), middleware.CORS())
	handler.RegisterRoutes(r, app)

	addr := ":" + cfg.Port
	slog.Info("go-backend listening", "addr", addr, "db", cfg.DBPath)
	if err := r.Run(addr); err != nil {
		slog.Error("server exit", "err", err)
		os.Exit(1)
	}
}
