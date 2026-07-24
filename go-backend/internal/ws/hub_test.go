package ws

import (
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/gorilla/websocket"
)

// 回归：管理端在线时广播不得持锁写连接，否则会与 writeConn 的 hub.mu.Lock 死锁，
// 卡死节点读循环 → 上线 SyncNodeConfig 全部超时 → 弃用 gost.json 后转发全空。
func TestBroadcastAdminsNoDeadlock(t *testing.T) {
	up := websocket.Upgrader{CheckOrigin: func(r *http.Request) bool { return true }}
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		c, err := up.Upgrade(w, r, nil)
		if err != nil {
			return
		}
		// 消费对端写入，避免阻塞
		go func() {
			defer c.Close()
			for {
				if _, _, err := c.ReadMessage(); err != nil {
					return
				}
			}
		}()
		// 保持连接直到测试结束由客户端关闭
		select {}
	}))
	defer srv.Close()

	wsURL := "ws" + srv.URL[len("http"):]
	admin, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("dial admin: %v", err)
	}
	defer admin.Close()

	h := NewHub()
	h.RegisterAdmin(admin, 1)

	done := make(chan struct{})
	go func() {
		// 连续广播应快速返回
		for i := 0; i < 20; i++ {
			h.BroadcastAdmins([]byte(`{"type":"status","data":1}`))
		}
		close(done)
	}()

	select {
	case <-done:
	case <-time.After(2 * time.Second):
		t.Fatal("BroadcastAdmins deadlocked (held RLock while writeConn tried Lock)")
	}
}

func TestHandleIncomingCallDoesNotBlock(t *testing.T) {
	up := websocket.Upgrader{CheckOrigin: func(r *http.Request) bool { return true }}
	var serverConn *websocket.Conn
	ready := make(chan struct{})
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		c, err := up.Upgrade(w, r, nil)
		if err != nil {
			return
		}
		serverConn = c
		close(ready)
		for {
			if _, _, err := c.ReadMessage(); err != nil {
				return
			}
		}
	}))
	defer srv.Close()

	wsURL := "ws" + srv.URL[len("http"):]
	client, _, err := websocket.DefaultDialer.Dial(wsURL, nil)
	if err != nil {
		t.Fatalf("dial: %v", err)
	}
	defer client.Close()
	<-ready

	h := NewHub()
	// 无 secret：明文 call
	done := make(chan struct{})
	go func() {
		h.HandleIncoming(serverConn, "", []byte(`{"uptime":1,"memory_usage":12.3}`))
		close(done)
	}()

	select {
	case <-done:
	case <-time.After(2 * time.Second):
		t.Fatal("HandleIncoming call reply blocked")
	}

	_ = client.SetReadDeadline(time.Now().Add(2 * time.Second))
	_, msg, err := client.ReadMessage()
	if err != nil {
		t.Fatalf("read call: %v", err)
	}
	if string(msg) != `{"type":"call"}` {
		t.Fatalf("unexpected call payload: %s", msg)
	}
}
