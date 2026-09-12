package service

import (
	"fmt"
	"log/slog"
	"regexp"
	"strconv"
	"strings"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/gost"
	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/repo"
	"github.com/Fearless743/flux-panel/go-backend/internal/ws"
	"github.com/google/uuid"
	"github.com/jmoiron/sqlx"
)

type NodeService struct {
	DB   *sqlx.DB
	Hub  *ws.Hub
	Repo *repo.NodeRepo
}

func NewNodeService(db *sqlx.DB, hub *ws.Hub) *NodeService {
	return &NodeService{
		DB:   db,
		Hub:  hub,
		Repo: repo.NewNodeRepo(db),
	}
}

type NodeCreateReq struct {
	Name          string  `json:"name"`
	ServerIP      string  `json:"serverIp"`
	AutoDetectIP  bool    `json:"autoDetectIP"`
	Port          string  `json:"port"`
	InterfaceName *string `json:"interfaceName"`
	TCPListenAddr string  `json:"tcpListenAddr"`
	UDPListenAddr string  `json:"udpListenAddr"`
}

type NodeUpdateReq struct {
	ID            int64   `json:"id"`
	Name          string  `json:"name"`
	ServerIP      string  `json:"serverIp"`
	AutoDetectIP  bool    `json:"autoDetectIP"`
	Port          string  `json:"port"`
	InterfaceName *string `json:"interfaceName"`
	HTTP          *int    `json:"http"`
	TLS           *int    `json:"tls"`
	Socks         *int    `json:"socks"`
	TCPListenAddr string  `json:"tcpListenAddr"`
	UDPListenAddr string  `json:"udpListenAddr"`
}

func (s *NodeService) Create(req NodeCreateReq) error {
	if strings.TrimSpace(req.Name) == "" {
		return fmt.Errorf("节点名称不能为空")
	}
	if strings.TrimSpace(req.ServerIP) == "" {
		return fmt.Errorf("服务器ip不能为空")
	}
	if strings.TrimSpace(req.Port) == "" {
		return fmt.Errorf("可用端口不能为空")
	}
	if err := ValidatePortRange(req.Port); err != nil {
		return err
	}
	n := &model.Node{
		Name:          req.Name,
		Secret:        strings.ReplaceAll(uuid.NewString(), "-", ""),
		ServerIP:      req.ServerIP,
		AutoDetectIP:  req.AutoDetectIP,
		Port:          req.Port,
		InterfaceName: req.InterfaceName,
		Status:        0,
		HTTP:          0,
		TLS:           0,
		Socks:         0,
		TCPListenAddr: req.TCPListenAddr,
		UDPListenAddr: req.UDPListenAddr,
	}
	return s.Repo.Create(n)
}

func (s *NodeService) List() ([]model.Node, error) {
	list, err := s.Repo.ListOrderByStatusDesc()
	if err != nil {
		return nil, err
	}
	for i := range list {
		list[i].Secret = ""
	}
	return list, nil
}

func (s *NodeService) Update(req NodeUpdateReq) error {
	if req.ID == 0 {
		return fmt.Errorf("节点ID不能为空")
	}
	if strings.TrimSpace(req.Name) == "" {
		return fmt.Errorf("节点名称不能为空")
	}
	if strings.TrimSpace(req.ServerIP) == "" {
		return fmt.Errorf("服务器ip不能为空")
	}
	if strings.TrimSpace(req.Port) == "" {
		return fmt.Errorf("可用port不能为空")
	}
	if err := ValidatePortRange(req.Port); err != nil {
		return err
	}

	node, err := s.Repo.GetByID(req.ID)
	if err != nil {
		return err
	}
	if node == nil {
		return fmt.Errorf("节点不存在")
	}

	online := node.Status == 1
	newHTTP := req.HTTP
	newTLS := req.TLS
	newSocks := req.Socks

	httpChanged := newHTTP != nil && *newHTTP != node.HTTP
	tlsChanged := newTLS != nil && *newTLS != node.TLS
	socksChanged := newSocks != nil && *newSocks != node.Socks

	if online && (httpChanged || tlsChanged || socksChanged) {
		payload := map[string]any{
			"http":  newHTTP,
			"tls":   newTLS,
			"socks": newSocks,
		}
		res := s.Hub.SendMsg(node.ID, payload, "SetProtocol")
		if !gost.IsOK(res.Msg) {
			return fmt.Errorf("%s", res.Msg)
		}
	}

	upd := &model.Node{
		ID:            req.ID,
		Name:          req.Name,
		ServerIP:      req.ServerIP,
		AutoDetectIP:  req.AutoDetectIP,
		Port:          req.Port,
		InterfaceName: req.InterfaceName,
		TCPListenAddr: req.TCPListenAddr,
		UDPListenAddr: req.UDPListenAddr,
	}
	if newHTTP != nil {
		upd.HTTP = *newHTTP
	} else {
		upd.HTTP = node.HTTP
	}
	if newTLS != nil {
		upd.TLS = *newTLS
	} else {
		upd.TLS = node.TLS
	}
	if newSocks != nil {
		upd.Socks = *newSocks
	} else {
		upd.Socks = node.Socks
	}
	return s.Repo.Update(upd)
}

func (s *NodeService) Delete(id int64) error {
	node, err := s.Repo.GetByID(id)
	if err != nil {
		return err
	}
	if node == nil {
		return fmt.Errorf("节点不存在")
	}
	// 从隧道拓扑剔除，禁止级联删隧道
	if err := DetachNodeFromTunnels(s.DB, s.Hub, id); err != nil {
		return err
	}
	return s.Repo.Delete(id)
}

func (s *NodeService) InstallCommand(id int64) (string, error) {
	node, err := s.Repo.GetByID(id)
	if err != nil {
		return "", err
	}
	if node == nil {
		return "", fmt.Errorf("节点不存在")
	}
	ip, err := s.Repo.GetViteConfig("ip")
	if err != nil {
		return "", err
	}
	if ip == "" {
		return "", fmt.Errorf("请先前往网站配置中设置ip")
	}
	processed := gost.ProcessServerAddress(ip)
	var b strings.Builder
	b.WriteString("curl -L https://raw.githubusercontent.com/Fearless743/flux-panel/refs/heads/beta/install.sh")
	b.WriteString(" -o ./install.sh && chmod +x ./install.sh && ")
	b.WriteString("./install.sh")
	b.WriteString(" -a ")
	b.WriteString(processed)
	b.WriteString(" -s ")
	b.WriteString(node.Secret)

	ssl, err := s.Repo.GetViteConfig("ssl")
	if err != nil {
		return "", err
	}
	if ssl == "true" {
		b.WriteString(" -l")
	}
	return b.String(), nil
}

// NodeUpgradeReq 远程升级请求
type NodeUpgradeReq struct {
	ID      int64  `json:"id"`
	Version string `json:"version"` // 空或 latest；也可为 tag 如 v1.2.3
	URL     string `json:"url"`     // 可选，覆盖默认 Release 地址（节点侧仍校验白名单）
	Sha256  string `json:"sha256"`  // 可选
	Arch    string `json:"arch"`    // 可选 amd64/arm64，默认由节点自检
}

// Upgrade 向在线节点下发 Upgrade 命令；节点下载校验替换后重启，失败自动 rollback
func (s *NodeService) Upgrade(req NodeUpgradeReq) (string, error) {
	if req.ID == 0 {
		return "", fmt.Errorf("节点ID不能为空")
	}
	if s.Hub == nil {
		return "", fmt.Errorf("WebSocket 未初始化")
	}
	node, err := s.Repo.GetByID(req.ID)
	if err != nil {
		return "", err
	}
	if node == nil {
		return "", fmt.Errorf("节点不存在")
	}
	if !s.Hub.IsNodeOnline(req.ID) {
		return "", fmt.Errorf("节点不在线，无法远程升级")
	}

	version := strings.TrimSpace(req.Version)
	if version == "" {
		version = "latest"
	}
	payload := map[string]any{
		"version": version,
	}
	if u := strings.TrimSpace(req.URL); u != "" {
		payload["url"] = u
	}
	if sum := strings.TrimSpace(req.Sha256); sum != "" {
		payload["sha256"] = sum
	}
	if arch := strings.TrimSpace(req.Arch); arch != "" {
		payload["arch"] = arch
	}

	// 节点侧异步下载；此处只等「任务已接受」
	res := s.Hub.SendMsgTimeout(req.ID, payload, "Upgrade", 15*time.Second)
	if !gost.IsOK(res.Msg) {
		return "", fmt.Errorf("%s", res.Msg)
	}
	msg := "升级任务已接受，节点将在后台下载并重启；失败时自动回滚到上一版本，请稍后刷新查看版本号"
	if node.Version != nil && *node.Version != "" {
		msg = fmt.Sprintf("当前版本 %s；%s", *node.Version, msg)
	}
	return msg, nil
}

func (s *NodeService) GetBySecret(secret string) (*model.Node, error) {
	return s.Repo.GetBySecret(secret)
}

func (s *NodeService) MarkOnline(id int64, version string, http, tls, socks string, clientIP string) error {
	var v *string
	if version != "" {
		v = &version
	}
	var h, t, so *int
	if http != "" {
		if n, err := strconv.Atoi(http); err == nil {
			h = &n
		}
	}
	if tls != "" {
		if n, err := strconv.Atoi(tls); err == nil {
			t = &n
		}
	}
	if socks != "" {
		if n, err := strconv.Atoi(socks); err == nil {
			so = &n
		}
	}
	if err := s.Repo.UpdateOnline(id, v, h, t, so); err != nil {
		return err
	}
	// 自动检测节点 IP：如果开启了 auto_detect_ip，记录本次连接的客户端 IP
	node, err := s.Repo.GetByID(id)
	if err == nil && node != nil && node.AutoDetectIP && clientIP != "" {
		if err := s.Repo.UpdateDetectedIP(id, clientIP); err != nil {
			slog.Warn("更新检测IP失败", "nodeId", id, "err", err)
		}
	}
	return nil
}

func (s *NodeService) MarkOffline(id int64) error {
	return s.Repo.UpdateStatus(id, 0)
}

// UpdateDetectedIP 直接更新节点的 detected_ip（由心跳中携带的 public_ip 触发）
func (s *NodeService) UpdateDetectedIP(id int64, detectedIP string) error {
	return s.Repo.UpdateDetectedIP(id, detectedIP)
}

// ValidatePortRange 对齐 Java validatePortRange
func ValidatePortRange(port string) error {
	portPattern := regexp.MustCompile(`^([0-9]{1,5})(-([0-9]{1,5}))?$`)
	if port == "" {
		return fmt.Errorf("可用端口不合法")
	}
	parts := strings.Split(port, ",")
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if !portPattern.MatchString(part) {
			return fmt.Errorf("可用端口不合法")
		}
		if strings.Contains(part, "-") {
			rangeParts := strings.Split(part, "-")
			start, _ := strconv.Atoi(rangeParts[0])
			end, _ := strconv.Atoi(rangeParts[1])
			if start < 0 || end < 0 || end > 65535 || start > end {
				return fmt.Errorf("可用端口不合法")
			}
		} else {
			p, _ := strconv.Atoi(part)
			if p < 0 || p > 65535 {
				return fmt.Errorf("可用端口不合法")
			}
		}
	}
	return nil
}
