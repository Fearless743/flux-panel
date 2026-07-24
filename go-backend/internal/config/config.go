package config

import (
	"os"
	"strconv"
)

type Config struct {
	Port      string
	DBPath    string
	JWTSecret string
	LogDir    string
}

func Load() Config {
	cfg := Config{
		Port:      getenv("PORT", "6365"),
		DBPath:    getenv("DB_PATH", "./data/gost.db"),
		JWTSecret: getenv("JWT_SECRET", "flux-panel-dev-secret"),
		LogDir:    getenv("LOG_DIR", "./logs"),
	}
	return cfg
}

func getenv(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

func GetenvInt(key string, def int) int {
	v := os.Getenv(key)
	if v == "" {
		return def
	}
	n, err := strconv.Atoi(v)
	if err != nil {
		return def
	}
	return n
}
