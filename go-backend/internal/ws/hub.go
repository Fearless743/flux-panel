package ws

import (
	"encoding/json"
	"log/slog"
	"net/http"
	"sync"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/crypto"
	"github.com/google/uuid"
	"github.com/gorilla/websocket"
)

var upgrader = websocket.Upgrader{
	CheckOrigin: func(r *http.Request) bool { return true },
}

// GostResult 对齐 Java GostDto
type GostResult struct {
	Msg  string          `json:"msg"`
	Data json.RawMessage `json:"data,omitempty"`
}

type sessionMeta struct {
	ID         int64 // nodeId 或 userId
	Type       string
	NodeSecret string
	Version    string
}

type Hub struct {
	mu             sync.RWMutex
	nodeSessions   map[int64]*websocket.Conn
	nodeMeta       map[int64]sessionMeta
	adminSessions  map[*websocket.Conn]sessionMeta
	sessionLocks   map[*websocket.Conn]*sync.Mutex
	pending        map[string]chan GostResult
	OnNodeOnline   func(nodeID int64, version, http, tls, socks string)
	OnNodeOffline  func(nodeID int64)
	OnAdminMessage func(nodeID int64, payload []byte) // 广播给管理端前可选处理
}

func NewHub() *Hub {
	return &Hub{
		nodeSessions:  make(map[int64]*websocket.Conn),
		nodeMeta:      make(map[int64]sessionMeta),
		adminSessions: make(map[*websocket.Conn]sessionMeta),
		sessionLocks:  make(map[*websocket.Conn]*sync.Mutex),
		pending:       make(map[string]chan GostResult),
	}
}

func (h *Hub) IsNodeOnline(nodeID int64) bool {
	h.mu.RLock()
	defer h.mu.RUnlock()
	c, ok := h.nodeSessions[nodeID]
	return ok && c != nil
}

// SendMsg 面板 → 节点，等待 requestId 响应，超时 10s
func (h *Hub) SendMsg(nodeID int64, data any, typ string) GostResult {
	return h.SendMsgTimeout(nodeID, data, typ, 10*time.Second)
}

// SendMsgTimeout 同 SendMsg，可自定义等待超时（如远程升级下载）
func (h *Hub) SendMsgTimeout(nodeID int64, data any, typ string, timeout time.Duration) GostResult {
	h.mu.RLock()
	conn := h.nodeSessions[nodeID]
	meta := h.nodeMeta[nodeID]
	h.mu.RUnlock()

	if conn == nil {
		return GostResult{Msg: "节点不在线"}
	}
	if timeout <= 0 {
		timeout = 10 * time.Second
	}

	reqID := uuid.NewString()
	ch := make(chan GostResult, 1)
	h.mu.Lock()
	h.pending[reqID] = ch
	h.mu.Unlock()
	defer func() {
		h.mu.Lock()
		delete(h.pending, reqID)
		h.mu.Unlock()
	}()

	payload := map[string]any{
		"type":      typ,
		"data":      data,
		"requestId": reqID,
	}
	raw, _ := json.Marshal(payload)
	if err := h.writeConn(conn, meta.NodeSecret, raw); err != nil {
		return GostResult{Msg: "发送消息失败: " + err.Error()}
	}

	select {
	case res := <-ch:
		return res
	case <-time.After(timeout):
		return GostResult{Msg: "等待响应超时"}
	}
}

func (h *Hub) writeConn(conn *websocket.Conn, secret string, plain []byte) error {
	h.mu.Lock()
	lk, ok := h.sessionLocks[conn]
	if !ok {
		lk = &sync.Mutex{}
		h.sessionLocks[conn] = lk
	}
	h.mu.Unlock()

	msg := plain
	if secret != "" {
		if c := crypto.GetOrCreate(secret); c != nil {
			enc, err := c.Encrypt(plain)
			if err == nil {
				wrap, _ := json.Marshal(map[string]any{
					"encrypted": true,
					"data":      enc,
					"timestamp": time.Now().UnixMilli(),
				})
				msg = wrap
			}
		}
	}

	lk.Lock()
	defer lk.Unlock()
	return conn.WriteMessage(websocket.TextMessage, msg)
}

func (h *Hub) BroadcastAdmins(message []byte) {
	h.mu.RLock()
	defer h.mu.RUnlock()
	for conn := range h.adminSessions {
		_ = h.writeConn(conn, "", message)
	}
}

func (h *Hub) RegisterNode(nodeID int64, conn *websocket.Conn, secret, version string) {
	h.mu.Lock()
	if old, ok := h.nodeSessions[nodeID]; ok && old != conn {
		_ = old.Close()
		delete(h.sessionLocks, old)
	}
	h.nodeSessions[nodeID] = conn
	h.nodeMeta[nodeID] = sessionMeta{ID: nodeID, Type: "1", NodeSecret: secret, Version: version}
	h.sessionLocks[conn] = &sync.Mutex{}
	h.mu.Unlock()
}

func (h *Hub) UnregisterNode(nodeID int64, conn *websocket.Conn) {
	h.mu.Lock()
	cur, ok := h.nodeSessions[nodeID]
	if ok && cur == conn {
		delete(h.nodeSessions, nodeID)
		delete(h.nodeMeta, nodeID)
	}
	delete(h.sessionLocks, conn)
	h.mu.Unlock()
}

func (h *Hub) RegisterAdmin(conn *websocket.Conn, userID int64) {
	h.mu.Lock()
	h.adminSessions[conn] = sessionMeta{ID: userID, Type: "0"}
	h.sessionLocks[conn] = &sync.Mutex{}
	h.mu.Unlock()
}

func (h *Hub) UnregisterAdmin(conn *websocket.Conn) {
	h.mu.Lock()
	delete(h.adminSessions, conn)
	delete(h.sessionLocks, conn)
	h.mu.Unlock()
}

func (h *Hub) HandleIncoming(conn *websocket.Conn, secret string, raw []byte) {
	payload := raw
	if secret != "" {
		if c := crypto.GetOrCreate(secret); c != nil {
			var wrap struct {
				Encrypted bool   `json:"encrypted"`
				Data      string `json:"data"`
			}
			if json.Unmarshal(raw, &wrap) == nil && wrap.Encrypted && wrap.Data != "" {
				if plain, err := c.Decrypt(wrap.Data); err == nil {
					payload = plain
				}
			}
		}
	}

	s := string(payload)
	if contains(s, "memory_usage") {
		_ = h.writeConn(conn, secret, []byte(`{"type":"call"}`))
		return
	}

	if contains(s, "requestId") {
		var resp struct {
			RequestID string          `json:"requestId"`
			Message   string          `json:"message"`
			Type      string          `json:"type"`
			Data      json.RawMessage `json:"data"`
		}
		if err := json.Unmarshal(payload, &resp); err != nil {
			slog.Warn("parse ws response", "err", err)
			return
		}
		if resp.RequestID != "" {
			h.mu.Lock()
			ch := h.pending[resp.RequestID]
			h.mu.Unlock()
			if ch != nil {
				msg := resp.Message
				if msg == "" {
					msg = "无响应消息"
				}
				select {
				case ch <- GostResult{Msg: msg, Data: resp.Data}:
				default:
				}
			}
		}
		return
	}
	slog.Info("ws message", "payload", truncate(s, 200))
}

func contains(s, sub string) bool {
	return len(s) >= len(sub) && (s == sub || len(sub) == 0 || indexOf(s, sub) >= 0)
}

func indexOf(s, sub string) int {
	for i := 0; i+len(sub) <= len(s); i++ {
		if s[i:i+len(sub)] == sub {
			return i
		}
	}
	return -1
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}
