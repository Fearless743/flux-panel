package main

import (
	"context"
	"flag"
	"fmt"
	"log"
	_ "net/http/pprof"
	"os"
	"os/exec"
	"runtime"
	"strings"
	"sync"

	"github.com/go-gost/core/logger"
	xlogger "github.com/go-gost/x/logger"
	"github.com/go-gost/x/service"
	"github.com/go-gost/x/socket"

	"github.com/go-gost/gost/internal/memtune"
	"github.com/judwhite/go-svc"
)

type stringList []string

func (l *stringList) String() string {
	return fmt.Sprintf("%s", *l)
}
func (l *stringList) Set(value string) error {
	*l = append(*l, value)
	return nil
}

var (
	cfgFile      string
	outputFormat string
	services     stringList
	nodes        stringList
	debug        bool
	trace        bool
	apiAddr      string
	metricsAddr  string
)

func init() {
	log.SetFlags(log.LstdFlags | log.Lshortfile | log.Lmicroseconds)

	args := strings.Join(os.Args[1:], "  ")

	if strings.Contains(args, " -- ") {
		var (
			wg  sync.WaitGroup
			ret int
		)

		ctx, cancel := context.WithCancel(context.Background())
		defer cancel()

		for wid, wargs := range strings.Split(" "+args+" ", " -- ") {
			wg.Add(1)
			go func(wid int, wargs string) {
				defer wg.Done()
				defer cancel()
				worker(wid, strings.Split(wargs, "  "), &ctx, &ret)
			}(wid, strings.TrimSpace(wargs))
		}

		wg.Wait()

		os.Exit(ret)
	}
}

// worker 以子进程方式运行分段参数（gost 的 " -- " 多实例语法）。
func worker(id int, args []string, ctx *context.Context, ret *int) {
	cmd := exec.CommandContext(*ctx, os.Args[0], args...)

	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Env = append(os.Environ(), fmt.Sprintf("_GOST_ID=%d", id))

	if err := cmd.Run(); err != nil {
		log.Fatal(err)
	}
	if cmd.ProcessState.Exited() {
		*ret = cmd.ProcessState.ExitCode()
	}
}

// loggerOption 支持 GOST_LOG_LEVEL 覆盖日志级别（trace/debug/info/warn/error/fatal）。
// 默认 info；高连接率节点可设为 warn 进一步减少每条连接的日志分配。
func loggerOption() xlogger.Option {
	level := strings.ToLower(strings.TrimSpace(os.Getenv("GOST_LOG_LEVEL")))
	switch level {
	case "trace", "debug", "info", "warn", "error", "fatal":
		return xlogger.LevelOption(logger.LogLevel(level))
	case "":
		return func(*xlogger.Options) {} // 保持默认 info，不返回 nil 以免调用空选项
	default:
		fmt.Printf("⚠️ GOST_LOG_LEVEL 无效: %q（已忽略，使用 info）\n", level)
		return func(*xlogger.Options) {}
	}
}

func init() {
	var printVersion bool

	flag.Var(&services, "L", "service list")
	flag.Var(&nodes, "F", "chain node list")
	flag.StringVar(&cfgFile, "C", "", "configuration file")
	flag.BoolVar(&printVersion, "V", false, "print version")
	flag.StringVar(&outputFormat, "O", "", "output format, one of yaml|json format")
	flag.BoolVar(&debug, "D", false, "debug mode")
	flag.BoolVar(&trace, "DD", false, "trace mode")
	flag.StringVar(&apiAddr, "api", "", "api service address")
	flag.StringVar(&metricsAddr, "metrics", "", "metrics service address")
	flag.Parse()

	if printVersion {
		fmt.Fprintf(os.Stdout, "gost %s (%s %s/%s)\n",
			version, runtime.Version(), runtime.GOOS, runtime.GOARCH)
		os.Exit(0)
	}
}

func main() {
	// 尽早调优 GC：默认 GOGC=100 会让堆涨到存活量的 2 倍才回收，
	// 小内存节点上 RSS 长期偏高；容器内部署时按 cgroup 上限设软限制。
	memtune.Tune()

	// 加载配置文件
	config, err := LoadConfig("config.json")
	if err != nil {
		fmt.Printf("❌ 配置加载失败: %v\n", err)
		fmt.Println("请确保当前目录存在 config.json 文件")
		os.Exit(1)
	}

	fmt.Printf("✅ 配置加载成功 - addr: %s\n", config.Addr)

	log := xlogger.NewLogger(loggerOption())
	logger.SetDefault(log)

	wsReporter := socket.StartWebSocketReporterWithConfig(config.Addr, config.Secret, config.Http, config.Tls, config.Socks, config.Ssl, version)
	defer wsReporter.Stop()
	service.SetHTTPReportURL(config.Addr, config.Secret)

	p := &program{}
	if err := svc.Run(p); err != nil {
		logger.Default().Fatal(err)
	}
}

// 构建时注入版本（与 Release tag 对齐），例如：
// VERSION=2.0.9-beta
// GOOS=linux GOARCH=amd64 go build -ldflags="-s -w -X main.version=${VERSION}" -o gost
// upx --best --lzma gost
