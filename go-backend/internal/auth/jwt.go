package auth

import (
	"crypto/hmac"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"strconv"
	"strings"
	"time"
)

// 对齐 Java JwtUtil：HmacSHA256，header.alg="HmacSHA256"，90 天，无 Bearer 前缀。

const expireDuration = 90 * 24 * time.Hour

type Claims struct {
	Sub    string `json:"sub"`
	Iat    int64  `json:"iat"`
	Exp    int64  `json:"exp"`
	User   string `json:"user"`
	Name   string `json:"name"`
	RoleID int    `json:"role_id"`
}

type JWT struct {
	secret string
}

func NewJWT(secret string) *JWT {
	return &JWT{secret: secret}
}

func (j *JWT) Generate(userID int64, username string, roleID int) (string, error) {
	now := time.Now()
	header := map[string]string{
		"alg": "HmacSHA256",
		"typ": "JWT",
	}
	headerJSON, _ := json.Marshal(header)
	payload := Claims{
		Sub:    strconv.FormatInt(userID, 10),
		Iat:    now.Unix(),
		Exp:    now.Add(expireDuration).Unix(),
		User:   username,
		Name:   username,
		RoleID: roleID,
	}
	payloadJSON, _ := json.Marshal(payload)

	encHeader := base64.RawURLEncoding.EncodeToString(headerJSON)
	encPayload := base64.RawURLEncoding.EncodeToString(payloadJSON)
	sig, err := j.sign(encHeader, encPayload)
	if err != nil {
		return "", err
	}
	return encHeader + "." + encPayload + "." + sig, nil
}

func (j *JWT) Validate(token string) bool {
	_, err := j.Parse(token)
	return err == nil
}

func (j *JWT) Parse(token string) (*Claims, error) {
	if token == "" {
		return nil, errors.New("empty token")
	}
	parts := strings.Split(token, ".")
	if len(parts) != 3 {
		return nil, errors.New("invalid token format")
	}
	expected, err := j.sign(parts[0], parts[1])
	if err != nil {
		return nil, err
	}
	if expected != parts[2] {
		return nil, errors.New("invalid signature")
	}
	raw, err := base64.RawURLEncoding.DecodeString(parts[1])
	if err != nil {
		return nil, err
	}
	var c Claims
	if err := json.Unmarshal(raw, &c); err != nil {
		return nil, err
	}
	if c.Exp <= time.Now().Unix() {
		return nil, errors.New("token expired")
	}
	return &c, nil
}

func (j *JWT) UserID(token string) (int64, error) {
	c, err := j.Parse(token)
	if err != nil {
		return 0, err
	}
	return strconv.ParseInt(c.Sub, 10, 64)
}

func (j *JWT) RoleID(token string) (int, error) {
	c, err := j.Parse(token)
	if err != nil {
		return 0, err
	}
	return c.RoleID, nil
}

func (j *JWT) sign(encHeader, encPayload string) (string, error) {
	mac := hmac.New(sha256.New, []byte(j.secret))
	if _, err := mac.Write([]byte(encHeader + "." + encPayload)); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(mac.Sum(nil)), nil
}
