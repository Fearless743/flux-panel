package tcpbrutal

import (
	"context"
	"net"
	"syscall"

	"github.com/go-gost/core/dialer"
	"github.com/go-gost/core/logger"
	md "github.com/go-gost/core/metadata"
	"github.com/go-gost/x/registry"
	"golang.org/x/sys/unix"
)

func init() {
	registry.DialerRegistry().Register("tcpbrutal", NewDialer)
}

type tcpBrutalDialer struct {
	md     metadata
	logger logger.Logger
}

func NewDialer(opts ...dialer.Option) dialer.Dialer {
	options := &dialer.Options{}
	for _, opt := range opts {
		opt(options)
	}

	return &tcpBrutalDialer{
		logger: options.Logger,
	}
}

func (d *tcpBrutalDialer) Init(md md.Metadata) (err error) {
	return d.parseMetadata(md)
}

func (d *tcpBrutalDialer) Dial(ctx context.Context, addr string, opts ...dialer.DialOption) (net.Conn, error) {
	var options dialer.DialOptions
	for _, opt := range opts {
		opt(&options)
	}

	network := "tcp"

	dialer := &net.Dialer{
		Control: func(network, address string, c syscall.RawConn) error {
			return c.Control(func(fd uintptr) {
				// 设置 brutal 拥塞控制
				if err := unix.SetsockoptString(int(fd), unix.IPPROTO_TCP, unix.TCP_CONGESTION, "brutal"); err != nil {
					d.logger.Warnf("failed to set TCP_CONGESTION brutal for dialer: %v", err)
				}
			})
		},
	}

	conn, err := dialer.DialContext(ctx, network, addr)
	if err != nil {
		d.logger.Error(err)
	}
	return conn, err
}
