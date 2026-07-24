package main

// version 由构建时 -ldflags 注入，例如：
//
//	go build -ldflags="-s -w -X main.version=2.0.9-beta" -o gost .
//
// 未注入时使用 dev，便于区分本地未打 tag 的构建。
var (
	version = "dev"
)
