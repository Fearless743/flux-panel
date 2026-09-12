package repo

import (
	"database/sql"
	"fmt"
	"strconv"

	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/jmoiron/sqlx"
)

type ChainTunnelRepo struct {
	DB *sqlx.DB
}

func NewChainTunnelRepo(db *sqlx.DB) *ChainTunnelRepo {
	return &ChainTunnelRepo{DB: db}
}

// chainType 在 schema 中为 VARCHAR，业务用 "1"/"2"/"3"
func chainTypeStr(t int) string {
	return strconv.Itoa(t)
}

func ParseChainType(s string) int {
	n, _ := strconv.Atoi(s)
	return n
}

func (r *ChainTunnelRepo) ListByTunnelID(tunnelID int64) ([]model.ChainTunnel, error) {
	var list []model.ChainTunnel
	err := r.DB.Select(&list, `SELECT * FROM chain_tunnel WHERE tunnel_id = ? ORDER BY id`, tunnelID)
	return list, err
}

func (r *ChainTunnelRepo) ListEntriesByTunnelID(tunnelID int64) ([]model.ChainTunnel, error) {
	var list []model.ChainTunnel
	err := r.DB.Select(&list, `SELECT * FROM chain_tunnel WHERE tunnel_id = ? AND chain_type = '1' ORDER BY id`, tunnelID)
	return list, err
}

func (r *ChainTunnelRepo) UpdateExitNodeIDs(id int64, exitNodeIDs *string) error {
	_, err := r.DB.Exec(`UPDATE chain_tunnel SET exit_node_ids = ? WHERE id = ?`, exitNodeIDs, id)
	return err
}

func (r *ChainTunnelRepo) ListByTunnelIDs(tunnelIDs []int64) ([]model.ChainTunnel, error) {
	if len(tunnelIDs) == 0 {
		return nil, nil
	}
	q, args, err := sqlx.In(`SELECT * FROM chain_tunnel WHERE tunnel_id IN (?) ORDER BY id`, tunnelIDs)
	if err != nil {
		return nil, err
	}
	q = r.DB.Rebind(q)
	var list []model.ChainTunnel
	err = r.DB.Select(&list, q, args...)
	return list, err
}

func (r *ChainTunnelRepo) ListByNodeID(nodeID int64) ([]model.ChainTunnel, error) {
	var list []model.ChainTunnel
	err := r.DB.Select(&list, `SELECT * FROM chain_tunnel WHERE node_id = ?`, nodeID)
	return list, err
}

// ListByNodeIDs 批量查询多个节点的链隧道（N+1 优化）
func (r *ChainTunnelRepo) ListByNodeIDs(nodeIDs []int64) ([]model.ChainTunnel, error) {
	if len(nodeIDs) == 0 {
		return nil, nil
	}
	q, args, err := sqlx.In(`SELECT * FROM chain_tunnel WHERE node_id IN (?)`, nodeIDs)
	if err != nil {
		return nil, err
	}
	q = r.DB.Rebind(q)
	var list []model.ChainTunnel
	if err := r.DB.Select(&list, q, args...); err != nil {
		return nil, err
	}
	return list, nil
}

func (r *ChainTunnelRepo) ListByNodeIDWithPort(nodeID int64) ([]model.ChainTunnel, error) {
	var list []model.ChainTunnel
	err := r.DB.Select(&list, `SELECT * FROM chain_tunnel WHERE node_id = ? AND port IS NOT NULL`, nodeID)
	return list, err
}

func (r *ChainTunnelRepo) InsertBatch(items []model.ChainTunnel) error {
	if len(items) == 0 {
		return nil
	}
	tx, err := r.DB.Beginx()
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	for i := range items {
		if err := insertChainTunnel(tx, &items[i]); err != nil {
			return err
		}
	}
	return tx.Commit()
}

func insertChainTunnel(ext sqlx.Ext, ct *model.ChainTunnel) error {
	res, err := ext.Exec(`
		INSERT INTO chain_tunnel (tunnel_id, chain_type, node_id, port, strategy, inx, protocol, brutal, exit_node_ids)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		ct.TunnelID, ct.ChainType, ct.NodeID, ct.Port, ct.Strategy, ct.Inx, ct.Protocol, boolToInt(ct.Brutal), ct.ExitNodeIDs,
	)
	if err != nil {
		return err
	}
	if ider, ok := res.(interface{ LastInsertId() (int64, error) }); ok {
		if id, err := ider.LastInsertId(); err == nil {
			ct.ID = id
		}
	}
	return nil
}

func (r *ChainTunnelRepo) DeleteByTunnelID(tunnelID int64) error {
	_, err := r.DB.Exec(`DELETE FROM chain_tunnel WHERE tunnel_id = ?`, tunnelID)
	return err
}

func (r *ChainTunnelRepo) DeleteByTunnelAndNode(tunnelID, nodeID int64) error {
	_, err := r.DB.Exec(`DELETE FROM chain_tunnel WHERE tunnel_id = ? AND node_id = ?`, tunnelID, nodeID)
	return err
}

func (r *ChainTunnelRepo) UsedPortsOnNode(nodeID int64) ([]int, error) {
	var ports []sql.NullInt64
	err := r.DB.Select(&ports, `SELECT port FROM chain_tunnel WHERE node_id = ? AND port IS NOT NULL`, nodeID)
	if err != nil {
		return nil, err
	}
	out := make([]int, 0, len(ports))
	for _, p := range ports {
		if p.Valid {
			out = append(out, int(p.Int64))
		}
	}
	return out, nil
}

func EnsureChainType(ct *model.ChainTunnel, typ int) {
	ct.ChainType = chainTypeStr(typ)
}

func NewChainTunnel(tunnelID int64, chainType int, nodeID int64, port *int, strategy, protocol *string, inx *int, brutal bool) model.ChainTunnel {
	return model.ChainTunnel{
		TunnelID:  tunnelID,
		ChainType: chainTypeStr(chainType),
		NodeID:    nodeID,
		Port:      port,
		Strategy:  strategy,
		Protocol:  protocol,
		Inx:       inx,
	}
}

func NewChainTunnelWithExitBinding(tunnelID int64, chainType int, nodeID int64, port *int, strategy, protocol *string, inx *int, brutal bool, exitNodeIDs *string) model.ChainTunnel {
	return model.ChainTunnel{
		TunnelID:    tunnelID,
		ChainType:   chainTypeStr(chainType),
		NodeID:      nodeID,
		Port:        port,
		Strategy:    strategy,
		Protocol:    protocol,
		Inx:         inx,
		ExitNodeIDs: exitNodeIDs,
	}
}

func IntPtr(v int) *int {
	return &v
}

func StringPtr(s string) *string {
	if s == "" {
		return nil
	}
	return &s
}

func ChainTypeOf(ct model.ChainTunnel) int {
	return ParseChainType(ct.ChainType)
}

func MustInt(s string) int {
	n, err := strconv.Atoi(s)
	if err != nil {
		return 0
	}
	return n
}

func FormatIDs(ids []int64) string {
	return fmt.Sprintf("%v", ids)
}
