package tcpbrutal

import (
	"context"
	"net"
	"syscall"

	"github.com/go-gost/core/limiter"
	"github.com/go-gost/core/listener"
	"github.com/go-gost/core/logger"
	md "github.com/go-gost/core/metadata"
	admission "github.com/go-gost/x/admission/wrapper"
	xnet "github.com/go-gost/x/internal/net"
	"github.com/go-gost/x/internal/net/proxyproto"
	climiter "github.com/go-gost/x/limiter/conn/wrapper"
	limiter_wrapper "github.com/go-gost/x/limiter/traffic/wrapper"
	metrics "github.com/go-gost/x/metrics/wrapper"
	stats "github.com/go-gost/x/observer/stats/wrapper"
	"github.com/go-gost/x/registry"
	"golang.org/x/sys/unix"
)

func init() {
	registry.ListenerRegistry().Register("tcpbrutal", NewListener)
}

type tcpBrutalListener struct {
	ln      net.Listener
	logger  logger.Logger
	md      metadata
	options listener.Options
}

func NewListener(opts ...listener.Option) listener.Listener {
	options := listener.Options{}
	for _, opt := range opts {
		opt(&options)
	}
	return &tcpBrutalListener{
		logger:  options.Logger,
		options: options,
	}
}

func (l *tcpBrutalListener) Init(md md.Metadata) (err error) {
	if err = l.parseMetadata(md); err != nil {
		return
	}

	network := "tcp"
	if xnet.IsIPv4(l.options.Addr) {
		network = "tcp4"
	}

	lc := net.ListenConfig{
		Control: func(network, address string, c syscall.RawConn) error {
			return c.Control(func(fd uintptr) {
				// 设置 brutal 拥塞控制
				if err := setTCPBrutal(int(fd), unix.IPPROTO_TCP, unix.TCP_CONGESTION, "brutal"); err != nil {
					l.logger.Warnf("failed to set TCP_CONGESTION brutal: %v", err)
				}
			})
		},
	}
	ln, err := lc.Listen(context.Background(), network, l.options.Addr)
	if err != nil {
		return
	}

	l.logger.Debugf("tcpbrutal listener started at %s", l.options.Addr)

	ln = proxyproto.WrapListener(l.options.ProxyProtocol, ln, 0)
	ln = metrics.WrapListener(l.options.Service, ln)
	ln = stats.WrapListener(ln, l.options.Stats)
	ln = admission.WrapListener(l.options.Admission, ln)
	ln = limiter_wrapper.WrapListener(l.options.Service, ln, l.options.TrafficLimiter)
	ln = climiter.WrapListener(l.options.ConnLimiter, ln)
	l.ln = ln

	return
}

func (l *tcpBrutalListener) Accept() (conn net.Conn, err error) {
	conn, err = l.ln.Accept()
	if err != nil {
		return
	}

	conn = limiter_wrapper.WrapConn(
		conn,
		l.options.TrafficLimiter,
		conn.RemoteAddr().String(),
		limiter.ScopeOption(limiter.ScopeConn),
		limiter.ServiceOption(l.options.Service),
		limiter.NetworkOption(conn.LocalAddr().Network()),
		limiter.SrcOption(conn.RemoteAddr().String()),
	)

	return
}

func (l *tcpBrutalListener) Addr() net.Addr {
	return l.ln.Addr()
}

func (l *tcpBrutalListener) Close() error {
	return l.ln.Close()
}

// setTCPBrutal 通过 setsockopt 设置 TCP 拥塞控制为 brutal
func setTCPBrutal(fd int, level, opt int, value string) error {
	// 使用 TCP_CONGESTION sockopt 设置拥塞控制算法
	buf := []byte(value)
	buf = append(buf, 0) // 以 null 结尾
	return unix.SetsockoptString(fd, level, opt, value)
}
