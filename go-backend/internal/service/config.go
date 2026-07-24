package service

import (
	"errors"
	"strings"

	"github.com/Fearless743/flux-panel/go-backend/internal/model"
	"github.com/Fearless743/flux-panel/go-backend/internal/repo"
	"github.com/jmoiron/sqlx"
)

type ConfigService struct {
	repo *repo.ConfigRepo
}

func NewConfigService(db *sqlx.DB) *ConfigService {
	return &ConfigService{repo: repo.NewConfigRepo(db)}
}

func (s *ConfigService) GetByName(name string) (*model.ViteConfig, error) {
	name = strings.TrimSpace(name)
	if name == "" {
		return nil, errors.New("配置名称不能为空")
	}
	c, err := s.repo.FindByName(name)
	if err != nil {
		return nil, err
	}
	if c == nil {
		return nil, errors.New("配置不存在")
	}
	return c, nil
}

func (s *ConfigService) ListMap() (map[string]string, error) {
	list, err := s.repo.ListAll()
	if err != nil {
		return nil, err
	}
	m := make(map[string]string, len(list))
	for _, c := range list {
		m[c.Name] = c.Value
	}
	return m, nil
}

func (s *ConfigService) UpdateMap(configMap map[string]string) error {
	if len(configMap) == 0 {
		return errors.New("配置数据不能为空")
	}
	for name, value := range configMap {
		if strings.TrimSpace(name) == "" {
			continue
		}
		if err := s.repo.Upsert(name, value); err != nil {
			return err
		}
	}
	return nil
}

func (s *ConfigService) UpdateSingle(name, value string) error {
	if strings.TrimSpace(name) == "" {
		return errors.New("配置名称不能为空")
	}
	if strings.TrimSpace(value) == "" {
		return errors.New("配置值不能为空")
	}
	return s.repo.Upsert(name, value)
}

func (s *ConfigService) IsCaptchaEnabled() bool {
	c, err := s.repo.FindByName("captcha_enabled")
	if err != nil || c == nil {
		return false
	}
	return c.Value == "true"
}
