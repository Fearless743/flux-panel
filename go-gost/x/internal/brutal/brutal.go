// Package brutal 封装 TCP Brutal 的 socket 参数设置（HyNetworks/tcp-brutal 内核模块）。
//
// TCP Brutal 与 BBR 不同：它不主动探测带宽，而是按配置的速率对数据包做 pacing。
// 因此必须通过 TCP_BRUTAL_PARAMS 显式下发速率（通常是接收方网络的下载带宽），
// 否则未设置速率的连接默认只按 1 Mbps 发送。
// 参见 https://github.com/HyNetworks/tcp-brutal/blob/master/README.zh.md
package brutal

import (
	"encoding/binary"
	"errors"
	"fmt"
	"unsafe"

	"golang.org/x/sys/unix"
)

const (
	// tcpBrutalParams 是 tcp-brutal 内核模块定义的 TCP_BRUTAL_PARAMS sockopt 编号
	tcpBrutalParams = 23301

	// defaultCwndGain 按官方文档建议取 2.0x。内核不支持浮点，以十分之一为单位，20 表示 2.0。
	defaultCwndGain uint32 = 20

	// DefaultRateMbps 面板未配置节点带宽(0)时的兜底速率，避免内核默认 1Mbps 过慢
	DefaultRateMbps uint32 = 100
)

// Enable 将 socket 的拥塞控制算法设置为 brutal，并写入速率参数。
// rateMbps 表示该连接应占用的带宽 (Mbps)；为 0 时使用 DefaultRateMbps 兜底。
// 遇到锁定规则（EPERM）时按官方文档跳过，由规则的速率生效。
func Enable(fd int, rateMbps uint32) error {
	if rateMbps <= 0 {
		rateMbps = DefaultRateMbps
	}

	if err := unix.SetsockoptString(fd, unix.IPPROTO_TCP, unix.TCP_CONGESTION, "brutal"); err != nil {
		if errors.Is(err, unix.EPERM) {
			// 拥塞算法被路由规则锁定；若已是 brutal 则直接发送即可（速率由规则决定）
			if cc, gerr := unix.GetsockoptString(fd, unix.IPPROTO_TCP, unix.TCP_CONGESTION); gerr == nil && cc == "brutal" {
				return nil
			}
			return fmt.Errorf("拥塞算法被锁定: %w", err)
		}
		return err
	}

	// v1 参数结构体（12 字节，v2 模块同样兼容）：
	// struct { u64 rate; u32 cwnd_gain; }，小端
	rate := uint64(rateMbps) * 1000000 / 8 // Mbps -> bytes/s
	var buf [12]byte
	binary.LittleEndian.PutUint64(buf[0:8], rate)
	binary.LittleEndian.PutUint32(buf[8:12], defaultCwndGain)

	var optlen uint = 12
	_, _, errno := unix.Syscall6(unix.SYS_SETSOCKOPT,
		uintptr(fd),
		uintptr(unix.IPPROTO_TCP),
		uintptr(tcpBrutalParams),
		uintptr(unsafe.Pointer(&buf[0])),
		uintptr(unsafe.Pointer(&optlen)),
		0,
	)
	if errno == 0 {
		return nil
	}
	switch {
	case errors.Is(errno, unix.EPERM), errors.Is(errno, unix.ENOPROTOOPT), errors.Is(errno, unix.EINVAL):
		// 锁定规则 / 模块未加载参数接口时，拥塞算法已启用，速率交给规则或默认值
		return nil
	default:
		return fmt.Errorf("设置 TCP_BRUTAL_PARAMS: %w", errno)
	}
}
