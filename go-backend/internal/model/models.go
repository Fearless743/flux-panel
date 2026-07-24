package model

// 表结构与 schema.sql / 现有 gost.db 对齐

type User struct {
	ID            int64  `db:"id" json:"id"`
	User          string `db:"user" json:"user"`
	Pwd           string `db:"pwd" json:"-"`
	RoleID        int    `db:"role_id" json:"roleId"`
	ExpTime       int64  `db:"exp_time" json:"expTime"`
	Flow          int64  `db:"flow" json:"flow"`
	InFlow        int64  `db:"in_flow" json:"inFlow"`
	OutFlow       int64  `db:"out_flow" json:"outFlow"`
	FlowResetTime int64  `db:"flow_reset_time" json:"flowResetTime"`
	Num           int    `db:"num" json:"num"`
	CreatedTime   int64  `db:"created_time" json:"createdTime"`
	UpdatedTime   *int64 `db:"updated_time" json:"updatedTime"`
	Status        int    `db:"status" json:"status"`
}

type Node struct {
	ID            int64   `db:"id" json:"id"`
	Name          string  `db:"name" json:"name"`
	Secret        string  `db:"secret" json:"secret"`
	ServerIP      string  `db:"server_ip" json:"serverIp"`
	Port          string  `db:"port" json:"port"`
	InterfaceName *string `db:"interface_name" json:"interfaceName"`
	Version       *string `db:"version" json:"version"`
	HTTP          int     `db:"http" json:"http"`
	TLS           int     `db:"tls" json:"tls"`
	Socks         int     `db:"socks" json:"socks"`
	CreatedTime   int64   `db:"created_time" json:"createdTime"`
	UpdatedTime   *int64  `db:"updated_time" json:"updatedTime"`
	Status        int     `db:"status" json:"status"`
	TCPListenAddr string  `db:"tcp_listen_addr" json:"tcpListenAddr"`
	UDPListenAddr string  `db:"udp_listen_addr" json:"udpListenAddr"`
}

type Tunnel struct {
	ID           int64    `db:"id" json:"id"`
	Name         string   `db:"name" json:"name"`
	TrafficRatio float64  `db:"traffic_ratio" json:"trafficRatio"`
	Type         int      `db:"type" json:"type"`
	Protocol     string   `db:"protocol" json:"protocol"`
	Flow         int      `db:"flow" json:"flow"`
	CreatedTime  int64    `db:"created_time" json:"createdTime"`
	UpdatedTime  int64    `db:"updated_time" json:"updatedTime"`
	Status       int      `db:"status" json:"status"`
	InIP         *string  `db:"in_ip" json:"inIp"`
}

type ChainTunnel struct {
	ID        int64   `db:"id" json:"id"`
	TunnelID  int64   `db:"tunnel_id" json:"tunnelId"`
	ChainType string  `db:"chain_type" json:"chainType"` // schema 为 VARCHAR，业务中 1入口/2转发链/3出口
	NodeID    int64   `db:"node_id" json:"nodeId"`
	Port      *int    `db:"port" json:"port"`
	Strategy  *string `db:"strategy" json:"strategy"`
	Inx       *int    `db:"inx" json:"inx"`
	Protocol  *string `db:"protocol" json:"protocol"`
}

type Forward struct {
	ID          int64  `db:"id" json:"id"`
	UserID      int    `db:"user_id" json:"userId"`
	UserName    string `db:"user_name" json:"userName"`
	Name        string `db:"name" json:"name"`
	TunnelID    int    `db:"tunnel_id" json:"tunnelId"`
	RemoteAddr  string `db:"remote_addr" json:"remoteAddr"`
	Strategy    string `db:"strategy" json:"strategy"`
	InFlow      int64  `db:"in_flow" json:"inFlow"`
	OutFlow     int64  `db:"out_flow" json:"outFlow"`
	CreatedTime int64  `db:"created_time" json:"createdTime"`
	UpdatedTime int64  `db:"updated_time" json:"updatedTime"`
	Status      int    `db:"status" json:"status"`
	Inx         int    `db:"inx" json:"inx"`
}

type ForwardPort struct {
	ID        int64 `db:"id" json:"id"`
	ForwardID int64 `db:"forward_id" json:"forwardId"`
	NodeID    int64 `db:"node_id" json:"nodeId"`
	Port      int   `db:"port" json:"port"`
}

type UserTunnel struct {
	ID            int    `db:"id" json:"id"`
	UserID        int    `db:"user_id" json:"userId"`
	TunnelID      int    `db:"tunnel_id" json:"tunnelId"`
	SpeedID       *int   `db:"speed_id" json:"speedId"`
	Num           int    `db:"num" json:"num"`
	Flow          int64  `db:"flow" json:"flow"`
	InFlow        int64  `db:"in_flow" json:"inFlow"`
	OutFlow       int64  `db:"out_flow" json:"outFlow"`
	FlowResetTime int64  `db:"flow_reset_time" json:"flowResetTime"`
	ExpTime       int64  `db:"exp_time" json:"expTime"`
	Status        int    `db:"status" json:"status"`
}

type SpeedLimit struct {
	ID          int64  `db:"id" json:"id"`
	Name        string `db:"name" json:"name"`
	Speed       int    `db:"speed" json:"speed"`
	TunnelID    int64  `db:"tunnel_id" json:"tunnelId"`
	TunnelName  string `db:"tunnel_name" json:"tunnelName"`
	CreatedTime int64  `db:"created_time" json:"createdTime"`
	UpdatedTime *int64 `db:"updated_time" json:"updatedTime"`
	Status      int    `db:"status" json:"status"`
}

type StatisticsFlow struct {
	ID          int64  `db:"id" json:"id"`
	UserID      int64  `db:"user_id" json:"userId"`
	Flow        int64  `db:"flow" json:"flow"`
	TotalFlow   int64  `db:"total_flow" json:"totalFlow"`
	Time        string `db:"time" json:"time"`
	CreatedTime int64  `db:"created_time" json:"createdTime"`
}

type ViteConfig struct {
	ID    int64  `db:"id" json:"id"`
	Name  string `db:"name" json:"name"`
	Value string `db:"value" json:"value"`
	Time  int64  `db:"time" json:"time"`
}
