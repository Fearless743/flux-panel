package socket

import (
	"net"
	"sync"
	"syscall"

	"golang.org/x/sys/unix"
)

var (
	brutalSupported bool
	brutalOnce      sync.Once
)

// CheckBrutalSupport 检测系统是否支持 TCP Brutal 拥塞控制
// 通过尝试设置 TCP_CONGESTION 为 brutal 来判断
func CheckBrutalSupport() bool {
	brutalOnce.Do(func() {
		// 尝试创建一个 TCP 监听器并设置拥塞控制
		lc := net.ListenConfig{
			Control: func(network, address string, c syscall.RawConn) error {
				return c.Control(func(fd uintptr) {
					// 尝试设置 brutal 拥塞控制
					if err := unix.SetsockoptString(int(fd), unix.IPPROTO_TCP, unix.TCP_CONGESTION, "brutal"); err != nil {
						brutalSupported = false
					} else {
						brutalSupported = true
					}
				})
			},
		}
		conn, err := lc.Listen(nil, "tcp", "127.0.0.1:0")
		if err != nil {
			return
		}
		conn.Close()
	})
	return brutalSupported
}

// GetBrutalSupport 获取 TCP Brutal 支持状态（缓存结果）
func GetBrutalSupport() bool {
	return brutalSupported
}
