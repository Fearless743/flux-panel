package auth

import (
	"crypto/md5"
	"encoding/hex"
	"strings"
)

// 对齐 Java Md5Util：密码 MD5 十六进制小写存储
func MD5(s string) string {
	sum := md5.Sum([]byte(s))
	return hex.EncodeToString(sum[:])
}

func CheckPassword(plain, hashed string) bool {
	return strings.EqualFold(MD5(plain), hashed)
}
