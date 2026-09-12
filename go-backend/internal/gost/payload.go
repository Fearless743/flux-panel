package gost

import (
	"fmt"
	"strings"
)

// 构造与 Java GostUtil 对齐的 JSON 友好 map 结构

func LimiterData(name int64, speed string) map[string]any {
	return map[string]any{
		"name":   fmt.Sprintf("%d", name),
		"limits": []string{fmt.Sprintf("$ %sMB %sMB", speed, speed)},
	}
}

func ProcessServerAddress(serverAddr string) string {
	if serverAddr == "" || strings.HasPrefix(serverAddr, "[") {
		return serverAddr
	}
	lastColon := strings.LastIndex(serverAddr, ":")
	if lastColon == -1 {
		if isIPv6(serverAddr) {
			return "[" + serverAddr + "]"
		}
		return serverAddr
	}
	host := serverAddr[:lastColon]
	port := serverAddr[lastColon:]
	if isIPv6(host) {
		return "[" + host + "]" + port
	}
	return serverAddr
}

func isIPv6(address string) bool {
	return strings.Count(address, ":") >= 2
}

// ChainHopNode 转发链 hop 内节点
type ChainNodeInput struct {
	Protocol string
	ServerIP string
	Port     int
	Brutal   bool // 是否启用 TCP Brutal 拥塞控制
}

func BuildChainData(tunnelID, sourceNodeID int64, interfaceName string, strategy string, nodes []ChainNodeInput) map[string]any {
	arr := make([]any, 0, len(nodes))
	for i, n := range nodes {
		arr = append(arr, map[string]any{
			"name": fmt.Sprintf("node_%d", i+1),
			"addr": ProcessServerAddress(fmt.Sprintf("%s:%d", n.ServerIP, n.Port)),
			"connector": map[string]any{
				"type": "relay",
			},
			"dialer": map[string]any{
				"type": func() string {
					if n.Brutal {
						return "tcpbrutal"
					}
					return n.Protocol
				}(),
			},
		})
	}
	if strategy == "" {
		strategy = "fifo"
	}
	hop := map[string]any{
		"name": fmt.Sprintf("hop_%d", tunnelID),
		"selector": map[string]any{
			"strategy":    strategy,
			"maxFails":    1,
			"failTimeout": int64(600000000000), // 600s 纳秒
		},
		"nodes": arr,
	}
	if interfaceName != "" {
		hop["interface"] = interfaceName
	}
	return map[string]any{
		"name": ChainName(tunnelID),
		"hops": []any{hop},
	}
}

func BuildForwardServices(baseName string, limiter *int, tcpListen, udpListen string, port int, interfaceName string, tunnelType int, tunnelID int64, remoteAddr, strategy string) []any {
	services := make([]any, 0, 2)
	for _, protocol := range []string{"tcp", "udp"} {
		addr := tcpListen
		if protocol == "udp" {
			addr = udpListen
		}
		svc := map[string]any{
			"name": baseName + "_" + protocol,
			"addr": fmt.Sprintf("%s:%d", addr, port),
		}
		if tunnelType == 1 && interfaceName != "" {
			svc["metadata"] = map[string]any{"interface": interfaceName}
		}
		if limiter != nil {
			svc["limiter"] = fmt.Sprintf("%d", *limiter)
		}
		handler := map[string]any{"type": protocol}
		if tunnelType == 2 {
			handler["chain"] = ChainName(tunnelID)
		}
		svc["handler"] = handler

		listener := map[string]any{"type": protocol}
		if protocol == "udp" {
			listener["metadata"] = map[string]any{"keepAlive": true}
		}
		svc["listener"] = listener
		svc["forwarder"] = buildForwarder(remoteAddr, strategy)
		services = append(services, svc)
	}
	return services
}

func buildForwarder(remoteAddr, strategy string) map[string]any {
	if strategy == "" {
		strategy = "fifo"
	}
	parts := strings.Split(remoteAddr, ",")
	nodes := make([]any, 0, len(parts))
	for i, addr := range parts {
		nodes = append(nodes, map[string]any{
			"name": fmt.Sprintf("node_%d", i+1),
			"addr": strings.TrimSpace(addr),
		})
	}
	return map[string]any{
		"nodes": nodes,
		"selector": map[string]any{
			"strategy":    strategy,
			"maxFails":    1,
			"failTimeout": "600s",
		},
	}
}

func BuildChainService(tunnelID int64, chainType int, protocol, tcpListen string, port int, interfaceName string, sourceNodeHasIface bool) []any {
	svc := map[string]any{
		"name": TunnelTLS(tunnelID),
		"addr": fmt.Sprintf("%s:%d", tcpListen, port),
	}
	// 只为出口节点(chainType=3)设置 interface
	if chainType == 3 && sourceNodeHasIface && interfaceName != "" {
		svc["metadata"] = map[string]any{"interface": interfaceName}
	}
	handler := map[string]any{"type": "relay"}
	if chainType == 2 {
		handler["chain"] = ChainName(tunnelID)
	}
	svc["handler"] = handler
	svc["listener"] = map[string]any{"type": protocol}
	return []any{svc}
}

func DeleteServicePayload(names []string) map[string]any {
	return map[string]any{"services": names}
}

func PauseResumePayload(baseName string) map[string]any {
	return map[string]any{
		"services": []string{baseName + "_tcp", baseName + "_udp"},
	}
}
