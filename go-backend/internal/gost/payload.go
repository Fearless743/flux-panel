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


func BuildForwardServices(baseName string, limiter *int, tcpListen, udpListen string, port int, interfaceName string, remoteAddr, strategy string) []any {
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
		if interfaceName != "" {
			svc["metadata"] = map[string]any{"interface": interfaceName}
		}
		if limiter != nil {
			svc["limiter"] = fmt.Sprintf("%d", *limiter)
		}
		handler := map[string]any{"type": protocol}
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


func DeleteServicePayload(names []string) map[string]any {
	return map[string]any{"services": names}
}

func PauseResumePayload(baseName string) map[string]any {
	return map[string]any{
		"services": []string{baseName + "_tcp", baseName + "_udp"},
	}
}
