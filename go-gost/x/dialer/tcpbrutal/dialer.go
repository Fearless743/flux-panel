package tcpbrutal

import (
	"context"
	"net"
	"syscall"

	"github.com/go-gost/core/dialer"
	"github.com/go-gost/core/logger"
	md "github.com/go-gost/core/metadata"
	"github.com/go-gost/x/internal/brutal"
	"github.com/go-gost/x/registry"
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

	// download 为目标网络（接收方）下载带宽 (Mbps)，作为 Brutal 发送速率；
	// 未配置(0)时 agent 兜底 100Mbps
	dialer := &net.Dialer{
		Control: func(network, address string, c syscall.RawConn) error {
			return c.Control(func(fd uintptr) {
				if err := brutal.Enable(int(fd), d.md.download); err != nil {
					d.logger.Warnf("failed to set tcp brutal for dialer: %v", err)
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
