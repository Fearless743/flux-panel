package service

import (
	"github.com/Fearless743/flux-panel/go-backend/internal/captcha"
	"github.com/jmoiron/sqlx"
)

type CaptchaService struct {
	cfg   *ConfigService
	store *captcha.Store
}

func NewCaptchaService(db *sqlx.DB) *CaptchaService {
	return &CaptchaService{
		cfg:   NewConfigService(db),
		store: captcha.Default(),
	}
}

func (s *CaptchaService) CheckEnabled() int {
	if s.cfg.IsCaptchaEnabled() {
		return 1
	}
	return 0
}

func (s *CaptchaService) Generate() (*captcha.GenerateResult, error) {
	return s.store.Generate()
}

// Verify 简化：接受 {id, code} 或 {id, data}
func (s *CaptchaService) Verify(id, code string) bool {
	return s.store.Verify(id, code)
}

func (s *CaptchaService) Secondary(id string) bool {
	return s.store.Secondary(id)
}
