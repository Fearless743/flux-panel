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

func (d *tcpBrutalDialer) parseMetadata(md md.Metadata) (err error) {
	d.md.download = uint32(mdutil.GetInt(md, "download"))
	d.md.upload = uint32(mdutil.GetInt(md, "upload"))

	return
}
