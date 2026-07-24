package repo

import (
	"database/sql"
	"time"

	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/jmoiron/sqlx"
)

type ConfigRepo struct {
	DB *sqlx.DB
}

func NewConfigRepo(db *sqlx.DB) *ConfigRepo {
	return &ConfigRepo{DB: db}
}

func (r *ConfigRepo) FindByName(name string) (*model.ViteConfig, error) {
	var c model.ViteConfig
	err := r.DB.Get(&c, `SELECT * FROM vite_config WHERE name = ? LIMIT 1`, name)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &c, nil
}

func (r *ConfigRepo) ListAll() ([]model.ViteConfig, error) {
	var list []model.ViteConfig
	err := r.DB.Select(&list, `SELECT * FROM vite_config ORDER BY id`)
	if err != nil {
		return nil, err
	}
	if list == nil {
		list = []model.ViteConfig{}
	}
	return list, nil
}

func (r *ConfigRepo) Upsert(name, value string) error {
	now := time.Now().UnixMilli()
	existing, err := r.FindByName(name)
	if err != nil {
		return err
	}
	if existing != nil {
		_, err = r.DB.Exec(`UPDATE vite_config SET value=?, time=? WHERE id=?`, value, now, existing.ID)
		return err
	}
	_, err = r.DB.Exec(`INSERT INTO vite_config (name, value, time) VALUES (?, ?, ?)`, name, value, now)
	return err
}
