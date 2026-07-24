package crypto

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"errors"
	"io"
	"sync"
)

// AES-256-GCM，密钥 = SHA-256(secret)，密文格式 nonce(12) + ciphertext+tag，Base64 标准编码。
// 与 Java AESCrypto / 节点端兼容。

const (
	gcmIVLength  = 12
	gcmTagLength = 16
)

type AESCrypto struct {
	gcm cipher.AEAD
}

func NewAESCrypto(secret string) (*AESCrypto, error) {
	if secret == "" {
		return nil, errors.New("密钥不能为空")
	}
	sum := sha256.Sum256([]byte(secret))
	block, err := aes.NewCipher(sum[:])
	if err != nil {
		return nil, err
	}
	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, err
	}
	return &AESCrypto{gcm: gcm}, nil
}

func (a *AESCrypto) Encrypt(data []byte) (string, error) {
	if len(data) == 0 {
		return "", errors.New("待加密数据不能为空")
	}
	nonce := make([]byte, gcmIVLength)
	if _, err := io.ReadFull(rand.Reader, nonce); err != nil {
		return "", err
	}
	// Seal appends ciphertext+tag to nonce prefix destination
	out := a.gcm.Seal(nonce, nonce, data, nil)
	return base64.StdEncoding.EncodeToString(out), nil
}

func (a *AESCrypto) EncryptString(s string) (string, error) {
	return a.Encrypt([]byte(s))
}

func (a *AESCrypto) Decrypt(encrypted string) ([]byte, error) {
	if encrypted == "" {
		return nil, errors.New("加密数据不能为空")
	}
	raw, err := base64.StdEncoding.DecodeString(encrypted)
	if err != nil {
		return nil, err
	}
	if len(raw) < gcmIVLength+gcmTagLength {
		return nil, errors.New("加密数据长度不足")
	}
	nonce := raw[:gcmIVLength]
	ciphertext := raw[gcmIVLength:]
	return a.gcm.Open(nil, nonce, ciphertext, nil)
}

func (a *AESCrypto) DecryptString(encrypted string) (string, error) {
	b, err := a.Decrypt(encrypted)
	if err != nil {
		return "", err
	}
	return string(b), nil
}

// 缓存加密器，避免重复创建
var (
	cryptoCache sync.Map // map[string]*AESCrypto
)

func GetOrCreate(secret string) *AESCrypto {
	if secret == "" {
		return nil
	}
	if v, ok := cryptoCache.Load(secret); ok {
		return v.(*AESCrypto)
	}
	c, err := NewAESCrypto(secret)
	if err != nil {
		return nil
	}
	actual, _ := cryptoCache.LoadOrStore(secret, c)
	return actual.(*AESCrypto)
}

func Clear(secret string) {
	cryptoCache.Delete(secret)
}
