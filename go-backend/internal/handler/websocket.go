package handler

import (
	"encoding/json"
	"log/slog"
	"net/http"
	"strconv"

	"github.com/Fearless743/flux-panel/go-backend/internal/crypto"
	"github.com/Fearless743/flux-panel/go-backend/internal/service"
	"github.com/gin-gonic/gin"
	"github.com/gorilla/websocket"
)

var wsUpgrader = websocket.Upgrader{
	CheckOrigin: func(r *http.Request) bool { return true },
}

// HandleWebSocket /system-info
// Query: secret, type, version, http, tls, socks
func (a *App) HandleWebSocket(c *gin.Context) {
	secret := c.Query("secret")
	typ := c.Query("type")
	version := c.Query("version")
	httpP := c.Query("http")
	tlsP := c.Query("tls")
	socksP := c.Query("socks")

	var (
		sessionID   int64
		nodeSecret  string
		isNode      bool
		nodeVersion string
	)

	if typ == "1" {
		// 节点连接：按 secret 查 node
		if secret == "" {
			c.Status(http.StatusUnauthorized)
			return
		}
		node, err := service.NewNodeService(a.DB, a.Hub).GetBySecret(secret)
		if err != nil || node == nil {
			slog.Info("节点验证失败：未找到匹配的secret")
			c.Status(http.StatusUnauthorized)
			return
		}
		sessionID = node.ID
		nodeSecret = secret
		isNode = true
		nodeVersion = version
		slog.Info("节点通过验证", "nodeId", node.ID, "version", version)
	} else {
		// 管理员：JWT 在 secret 字段
		if secret == "" || !a.JWT.Validate(secret) {
			c.Status(http.StatusUnauthorized)
			return
		}
		uid, err := a.JWT.UserID(secret)
		if err != nil {
			c.Status(http.StatusUnauthorized)
			return
		}
		sessionID = uid
		isNode = false
	}

	conn, err := wsUpgrader.Upgrade(c.Writer, c.Request, nil)
	if err != nil {
		slog.Warn("websocket upgrade", "err", err)
		return
	}

	if isNode {
		a.Hub.RegisterNode(sessionID, conn, nodeSecret, nodeVersion)
		// 更新在线状态
		if err := service.NewNodeService(a.DB, a.Hub).MarkOnline(sessionID, version, httpP, tlsP, socksP); err != nil {
			slog.Warn("节点状态更新失败", "nodeId", sessionID, "err", err)
		} else {
			slog.Info("节点连接建立成功", "nodeId", sessionID, "version", version)
			broadcastStatus(a, sessionID, 1)
			// 上线后按 DB 全量重放期望配置（异步，不阻塞读循环）
			go service.NewNodeSyncService(a.DB, a.Hub).SyncNodeConfig(sessionID)
		}
	} else {
		a.Hub.RegisterAdmin(conn, sessionID)
		slog.Info("管理员连接建立", "userId", sessionID)
	}

	defer func() {
		if isNode {
			a.Hub.UnregisterNode(sessionID, conn)
			// 仅当仍是当前 session 时 UnregisterNode 会清 map；再查是否在线
			if !a.Hub.IsNodeOnline(sessionID) {
				if err := service.NewNodeService(a.DB, a.Hub).MarkOffline(sessionID); err != nil {
					slog.Warn("节点离线状态更新失败", "nodeId", sessionID, "err", err)
				} else {
					slog.Info("节点状态更新为离线", "nodeId", sessionID)
					broadcastStatus(a, sessionID, 0)
				}
			} else {
				slog.Info("节点连接关闭但已有新连接，跳过状态更新", "nodeId", sessionID)
			}
		} else {
			a.Hub.UnregisterAdmin(conn)
			slog.Info("管理员连接关闭", "userId", sessionID)
		}
		_ = conn.Close()
	}()

	for {
		_, raw, err := conn.ReadMessage()
		if err != nil {
			if !websocket.IsCloseError(err, websocket.CloseGoingAway, websocket.CloseNormalClosure, websocket.CloseNoStatusReceived) {
				slog.Debug("ws read", "err", err)
			}
			break
		}

		// 解密副本用于广播
		plain := decryptIfNeeded(nodeSecret, raw)
		a.Hub.HandleIncoming(conn, nodeSecret, raw)

		if isNode {
			msg, _ := json.Marshal(map[string]any{
				"id":   strconv.FormatInt(sessionID, 10),
				"type": "info",
				"data": string(plain),
			})
			a.Hub.BroadcastAdmins(msg)
		}
	}
}

func broadcastStatus(a *App, nodeID int64, status int) {
	msg, _ := json.Marshal(map[string]any{
		"id":   strconv.FormatInt(nodeID, 10),
		"type": "status",
		"data": status,
	})
	a.Hub.BroadcastAdmins(msg)
}

func decryptIfNeeded(secret string, raw []byte) []byte {
	if secret == "" {
		return raw
	}
	c := crypto.GetOrCreate(secret)
	if c == nil {
		return raw
	}
	var wrap struct {
		Encrypted bool   `json:"encrypted"`
		Data      string `json:"data"`
	}
	if json.Unmarshal(raw, &wrap) == nil && wrap.Encrypted && wrap.Data != "" {
		if plain, err := c.Decrypt(wrap.Data); err == nil {
			return plain
		}
	}
	return raw
}
