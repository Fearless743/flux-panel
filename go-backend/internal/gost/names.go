package gost

import (
	"fmt"
	"strconv"
	"strings"
)

// 服务命名约定 — 节点对账依赖，不可改

func ForwardServiceBase(forwardID, userID, userTunnelID int64) string {
	return fmt.Sprintf("%d_%d_%d", forwardID, userID, userTunnelID)
}

func ForwardTCP(base string) string { return base + "_tcp" }
func ForwardUDP(base string) string { return base + "_udp" }

func TunnelTLS(tunnelID int64) string {
	return fmt.Sprintf("%d_tls", tunnelID)
}

func ChainName(tunnelID int64) string {
	return fmt.Sprintf("chains_%d", tunnelID)
}

// ParseForwardServiceName 解析 n 字段: "{forwardId}_{userId}_{userTunnelId}_tcp|udp"
// 返回 forwardID, userID, userTunnelID, ok
func ParseForwardServiceName(n string) (forwardID, userID, userTunnelID int64, ok bool) {
	if n == "" || n == "web_api" {
		return 0, 0, 0, false
	}
	// 去掉 _tcp / _udp
	base := n
	if strings.HasSuffix(n, "_tcp") {
		base = strings.TrimSuffix(n, "_tcp")
	} else if strings.HasSuffix(n, "_udp") {
		base = strings.TrimSuffix(n, "_udp")
	}
	parts := strings.Split(base, "_")
	if len(parts) < 3 {
		return 0, 0, 0, false
	}
	// 最后三段
	p := parts[len(parts)-3:]
	f, err1 := strconv.ParseInt(p[0], 10, 64)
	u, err2 := strconv.ParseInt(p[1], 10, 64)
	t, err3 := strconv.ParseInt(p[2], 10, 64)
	if err1 != nil || err2 != nil || err3 != nil {
		return 0, 0, 0, false
	}
	return f, u, t, true
}

// IsMissing 节点侧资源不存在（Update/Delete 常见）；空节点同步时应回落 Add，不能当成功。
func IsMissing(msg string) bool {
	return strings.Contains(msg, "not found")
}

// IsExists 节点侧资源已存在（Add 幂等成功）。
func IsExists(msg string) bool {
	return strings.Contains(msg, "exists")
}

// IsOK 判定下发结果是否可视为成功。
// - "OK"：明确成功
// - "exists"：Add 时已存在，幂等成功
// 注意：不再把 "not found" 当成功（弃用 gost.json 后 Update 空节点会误判，导致不同步端口）。
func IsOK(msg string) bool {
	return msg == "OK" || IsExists(msg)
}

// NormalizeOK 将幂等成功文案归一为 OK；not found 保持原样，便于调用方回落 Add。
func NormalizeOK(msg string) string {
	if IsExists(msg) {
		return "OK"
	}
	return msg
}
