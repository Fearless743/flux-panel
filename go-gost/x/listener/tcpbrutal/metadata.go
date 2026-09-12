package tcpbrutal

import (
	md "github.com/go-gost/core/metadata"
	mdutil "github.com/go-gost/x/metadata/util"
)

type metadata struct {
	// download 下载带宽 (Mbps)
	download uint32
	// upload 上传带宽 (Mbps)
	upload uint32
}

func (l *tcpBrutalListener) parseMetadata(md md.Metadata) (err error) {
	l.md.download = uint32(mdutil.GetInt(md, "download"))
	l.md.upload = uint32(mdutil.GetInt(md, "upload"))

	return
}
