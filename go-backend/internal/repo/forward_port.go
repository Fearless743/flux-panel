package repo

import (
	"database/sql"

	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/jmoiron/sqlx"
)

type ForwardPortRepo struct {
	DB *sqlx.DB
}

func NewForwardPortRepo(db *sqlx.DB) *ForwardPortRepo {
	return &ForwardPortRepo{DB: db}
}

func (r *ForwardPortRepo) ListByForwardID(forwardID int64) ([]model.ForwardPort, error) {
	var list []model.ForwardPort
	err := r.DB.Select(&list, `SELECT id, forward_id, node_id, port FROM forward_port WHERE forward_id=?`, forwardID)
	return list, err
}

func (r *ForwardPortRepo) GetByForwardAndNode(forwardID, nodeID int64) (*model.ForwardPort, error) {
	var fp model.ForwardPort
	err := r.DB.Get(&fp, `SELECT id, forward_id, node_id, port FROM forward_port WHERE forward_id=? AND node_id=?`,
		forwardID, nodeID)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &fp, nil
}

func (r *ForwardPortRepo) ListByNodeExcludeForward(nodeID, excludeForwardID int64) ([]model.ForwardPort, error) {
	var list []model.ForwardPort
	err := r.DB.Select(&list, `SELECT id, forward_id, node_id, port FROM forward_port WHERE node_id=? AND forward_id<>?`,
		nodeID, excludeForwardID)
	return list, err
}

func (r *ForwardPortRepo) Insert(fp *model.ForwardPort) (int64, error) {
	res, err := r.DB.Exec(`INSERT INTO forward_port (forward_id, node_id, port) VALUES (?, ?, ?)`,
		fp.ForwardID, fp.NodeID, fp.Port)
	if err != nil {
		return 0, err
	}
	return res.LastInsertId()
}

func (r *ForwardPortRepo) UpdatePort(id int64, port int) error {
	_, err := r.DB.Exec(`UPDATE forward_port SET port=? WHERE id=?`, port, id)
	return err
}

func (r *ForwardPortRepo) DeleteByForwardID(forwardID int64) error {
	_, err := r.DB.Exec(`DELETE FROM forward_port WHERE forward_id=?`, forwardID)
	return err
}

func (r *ForwardPortRepo) ListByNodeID(nodeID int64) ([]model.ForwardPort, error) {
	var list []model.ForwardPort
	err := r.DB.Select(&list, `SELECT id, forward_id, node_id, port FROM forward_port WHERE node_id=?`, nodeID)
	return list, err
}

func (r *ForwardPortRepo) UsedPortsOnNode(nodeID int64) ([]int, error) {
	var ports []int
	err := r.DB.Select(&ports, `SELECT port FROM forward_port WHERE node_id=?`, nodeID)
	return ports, err
}

func (r *ForwardPortRepo) DeleteByForwardAndNode(forwardID, nodeID int64) error {
	_, err := r.DB.Exec(`DELETE FROM forward_port WHERE forward_id=? AND node_id=?`, forwardID, nodeID)
	return err
}

func (r *ForwardPortRepo) DeleteByNodeID(nodeID int64) error {
	_, err := r.DB.Exec(`DELETE FROM forward_port WHERE node_id=?`, nodeID)
	return err
}

func (r *ForwardPortRepo) DeleteByNodeAndForwardIDs(nodeID int64, forwardIDs []int64) error {
	if len(forwardIDs) == 0 {
		return nil
	}
	q, args, err := sqlx.In(`DELETE FROM forward_port WHERE node_id=? AND forward_id IN (?)`, nodeID, forwardIDs)
	if err != nil {
		return err
	}
	q = r.DB.Rebind(q)
	_, err = r.DB.Exec(q, args...)
	return err
}

func (r *ForwardPortRepo) InsertOne(fp *model.ForwardPort) error {
	id, err := r.Insert(fp)
	if err != nil {
		return err
	}
	fp.ID = id
	return nil
}
