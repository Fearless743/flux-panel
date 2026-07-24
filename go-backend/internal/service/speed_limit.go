package service

import (
	"fmt"
	"math"
	"strconv"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/gost"
	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/repo"
	"github.com/Fearless743/flux-panel/go-backend/internal/ws"
	"github.com/jmoiron/sqlx"
)

type SpeedLimitService struct {
	repo *repo.SpeedLimitRepo
	hub  *ws.Hub
}

func NewSpeedLimitService(db *sqlx.DB, hub *ws.Hub) *SpeedLimitService {
	return &SpeedLimitService{
		repo: repo.NewSpeedLimitRepo(db),
		hub:  hub,
	}
}

type CreateSpeedLimitReq struct {
	Name       string `json:"name"`
	Speed      int    `json:"speed"`
	TunnelID   int64  `json:"tunnelId"`
	TunnelName string `json:"tunnelName"`
}

type UpdateSpeedLimitReq struct {
	ID    int64  `json:"id"`
	Name  string `json:"name"`
	Speed int    `json:"speed"`
}

func (s *SpeedLimitService) Create(req CreateSpeedLimitReq) error {
	if req.Name == "" {
		return fmt.Errorf("限速规则名称不能为空")
	}
	if req.Speed < 1 {
		return fmt.Errorf("速度限制必须大于0")
	}
	if req.TunnelID == 0 {
		return fmt.Errorf("隧道ID不能为空")
	}
	if req.TunnelName == "" {
		return fmt.Errorf("隧道名称不能为空")
	}

	tunnel, err := s.repo.GetTunnelByID(req.TunnelID)
	if err != nil {
		return fmt.Errorf("查询隧道失败: %v", err)
	}
	if tunnel == nil {
		return fmt.Errorf("隧道不存在")
	}

	now := time.Now().UnixMilli()
	sl := &model.SpeedLimit{
		Name:        req.Name,
		Speed:       req.Speed,
		TunnelID:    req.TunnelID,
		TunnelName:  req.TunnelName,
		CreatedTime: now,
		UpdatedTime: &now,
		Status:      1,
	}
	id, err := s.repo.Create(sl)
	if err != nil {
		return fmt.Errorf("保存限速规则失败: %v", err)
	}
	sl.ID = id

	speedInMBps := convertBitsToMBps(sl.Speed)
	nodeIDs, err := s.repo.ListChainNodeIDs(sl.TunnelID)
	if err != nil {
		_ = s.repo.Delete(sl.ID)
		return fmt.Errorf("查询隧道节点失败: %v", err)
	}

	success := make([]int64, 0, len(nodeIDs))
	for _, nodeID := range nodeIDs {
		res := s.hub.SendMsg(nodeID, gost.LimiterData(sl.ID, speedInMBps), "AddLimiters")
		if gost.IsOK(res.Msg) {
			success = append(success, nodeID)
			continue
		}
		// 失败回滚 DB + 已成功节点的 DeleteLimiters
		_ = s.repo.Delete(sl.ID)
		for _, nid := range success {
			_ = s.hub.SendMsg(nid, map[string]any{"limiter": strconv.FormatInt(sl.ID, 10)}, "DeleteLimiters")
		}
		return fmt.Errorf("%s", gost.NormalizeOK(res.Msg))
	}
	return nil
}

func (s *SpeedLimitService) List() ([]model.SpeedLimit, error) {
	return s.repo.List()
}

func (s *SpeedLimitService) Update(req UpdateSpeedLimitReq) error {
	if req.ID == 0 {
		return fmt.Errorf("ID不能为空")
	}
	if req.Name == "" {
		return fmt.Errorf("限速规则名称不能为空")
	}
	if req.Speed < 1 {
		return fmt.Errorf("速度限制必须大于0")
	}

	sl, err := s.repo.GetByID(req.ID)
	if err != nil {
		return fmt.Errorf("查询限速规则失败: %v", err)
	}
	if sl == nil {
		return fmt.Errorf("限速不存在")
	}

	sl.Name = req.Name
	sl.Speed = req.Speed
	now := time.Now().UnixMilli()
	sl.UpdatedTime = &now

	speedInMBps := convertBitsToMBps(sl.Speed)
	nodeIDs, err := s.repo.ListChainNodeIDs(sl.TunnelID)
	if err != nil {
		return fmt.Errorf("查询隧道节点失败: %v", err)
	}
	for _, nodeID := range nodeIDs {
		payload := map[string]any{
			"limiter": strconv.FormatInt(sl.ID, 10),
			"data":    gost.LimiterData(sl.ID, speedInMBps),
		}
		res := s.hub.SendMsg(nodeID, payload, "UpdateLimiters")
		// Java 对 Update 不做 exists/not found 归一，仅接受 OK
		if res.Msg != "OK" {
			return fmt.Errorf("%s", res.Msg)
		}
	}

	if err := s.repo.Update(sl); err != nil {
		return fmt.Errorf("更新限速规则失败: %v", err)
	}
	return nil
}

func (s *SpeedLimitService) Delete(id int64) error {
	if id == 0 {
		return fmt.Errorf("ID不能为空")
	}
	sl, err := s.repo.GetByID(id)
	if err != nil {
		return fmt.Errorf("查询限速规则失败: %v", err)
	}
	if sl == nil {
		return fmt.Errorf("限速规则不存在")
	}

	cnt, err := s.repo.CountUserTunnelBySpeedID(sl.ID)
	if err != nil {
		return fmt.Errorf("检查引用失败: %v", err)
	}
	if cnt != 0 {
		return fmt.Errorf("该限速规则还有用户在使用 请先取消分配")
	}

	nodeIDs, err := s.repo.ListChainNodeIDs(sl.TunnelID)
	if err != nil {
		return fmt.Errorf("查询隧道节点失败: %v", err)
	}
	for _, nodeID := range nodeIDs {
		res := s.hub.SendMsg(nodeID, map[string]any{"limiter": strconv.FormatInt(sl.ID, 10)}, "DeleteLimiters")
		if !gost.IsOK(res.Msg) {
			return fmt.Errorf("%s", res.Msg)
		}
	}

	if err := s.repo.Delete(id); err != nil {
		return fmt.Errorf("删除限速规则失败: %v", err)
	}
	return nil
}

func (s *SpeedLimitService) ListTunnels() ([]model.Tunnel, error) {
	return s.repo.ListAllTunnels()
}

// convertBitsToMBps speed(Mbps 量级，Java 命名 bits) / 8 → MB/s，一位小数
func convertBitsToMBps(speedInBits int) string {
	mbs := float64(speedInBits) / 8.0
	rounded := math.Round(mbs*10) / 10
	return strconv.FormatFloat(rounded, 'f', 1, 64)
}
