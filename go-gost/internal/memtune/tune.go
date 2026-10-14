// Package memtune 提供 gost 节点的 Go 运行时内存调优：
// 依据 cgroup 内存上限或用户指定的环境变量设置 GC 行为，压低 RSS。
package memtune

import (
	"fmt"
	"os"
	"runtime/debug"
	"strconv"
	"strings"
)

// Tune 调优 Go 运行时内存行为，应在 main() 最开始调用。
//
// - GOST_GOMEMLIMIT：显式软内存上限，如 "512MiB"、"1G"、纯字节数。
//   未设置时自动读取 cgroup 内存上限，取其 90% 作为软上限
//   （Docker/systemd 部署均生效），避免 GOGC=100 默认策略下堆涨到
//   存活内存 2 倍、RSS 在小内存节点上长期偏高的问题。
// - GOST_GOGC：覆盖 GOGC（默认 100；"off" 表示不按堆增长触发 GC，
//   完全依赖 GOMEMLIMIT 压制堆）。
func Tune() {
	if v := strings.TrimSpace(os.Getenv("GOST_GOGC")); v != "" {
		if strings.EqualFold(v, "off") {
			debug.SetGCPercent(-1)
			fmt.Println("🧹 GC: GOGC=off (GOST_GOGC)")
		} else if n, err := strconv.Atoi(v); err == nil && n > 0 {
			debug.SetGCPercent(n)
			fmt.Printf("🧹 GC: GOGC=%d (GOST_GOGC)\n", n)
		} else {
			fmt.Printf("⚠️ GOST_GOGC 无效: %q（已忽略）\n", v)
		}
	}

	if v := strings.TrimSpace(os.Getenv("GOST_GOMEMLIMIT")); v != "" {
		if limit, ok := ParseByteSize(v); ok {
			debug.SetMemoryLimit(limit)
			fmt.Printf("🧠 内存软上限: %s (GOST_GOMEMLIMIT)\n", FormatByteSize(limit))
			return
		}
		fmt.Printf("⚠️ GOST_GOMEMLIMIT 无效: %q，回退到 cgroup 自动探测\n", v)
	}

	if limit := DetectCgroupMemoryLimit(); limit > 0 {
		soft := limit - limit/10 // 留 10% 给栈/运行时等非堆内存
		debug.SetMemoryLimit(soft)
		fmt.Printf("🧠 检测到容器内存上限 %s，设置内存软上限 %s (90%%)\n",
			FormatByteSize(limit), FormatByteSize(soft))
	}
}

// DetectCgroupMemoryLimit 返回当前进程的 cgroup 内存上限（字节），无限制返回 0。
func DetectCgroupMemoryLimit() int64 {
	// cgroup v2：/sys/fs/cgroup/memory.max，"max" 表示无限制
	if b, err := os.ReadFile("/sys/fs/cgroup/memory.max"); err == nil {
		if v, ok := ParseByteSize(strings.TrimSpace(string(b))); ok {
			return v
		}
	}
	// cgroup v1：无限制时是一个接近 int64 最大值的哨兵值，需过滤
	if b, err := os.ReadFile("/sys/fs/cgroup/memory/memory.limit_in_bytes"); err == nil {
		if v, err := strconv.ParseInt(strings.TrimSpace(string(b)), 10, 64); err == nil {
			if v > 0 && v < (int64(1)<<62) {
				return v
			}
		}
	}
	return 0
}

// ParseByteSize 解析 "512MiB" / "1G" / "1024" 等字节表示，失败返回 false。
func ParseByteSize(s string) (int64, bool) {
	s = strings.TrimSpace(s)
	if s == "" {
		return 0, false
	}
	// 长后缀优先，避免 "KiB" 被 "K" 抢先匹配
	units := []struct {
		suffix string
		mult   float64
	}{
		{"KiB", 1 << 10}, {"MiB", 1 << 20}, {"GiB", 1 << 30},
		{"KB", 1e3}, {"MB", 1e6}, {"GB", 1e9},
		{"K", 1 << 10}, {"M", 1 << 20}, {"G", 1 << 30},
		{"B", 1},
	}
	for _, u := range units {
		if len(s) > len(u.suffix) && strings.HasSuffix(s, u.suffix) {
			n, err := strconv.ParseFloat(strings.TrimSpace(strings.TrimSuffix(s, u.suffix)), 64)
			if err != nil || n <= 0 {
				return 0, false
			}
			return int64(n * u.mult), true
		}
	}
	n, err := strconv.ParseInt(s, 10, 64)
	if err != nil || n <= 0 {
		return 0, false
	}
	return n, true
}

// FormatByteSize 以 MiB/GiB 为单位格式化字节数，仅用于启动日志。
func FormatByteSize(n int64) string {
	switch {
	case n >= 1<<30:
		return fmt.Sprintf("%.1fGiB", float64(n)/(1<<30))
	case n >= 1<<20:
		return fmt.Sprintf("%.0fMiB", float64(n)/(1<<20))
	default:
		return fmt.Sprintf("%dB", n)
	}
}
