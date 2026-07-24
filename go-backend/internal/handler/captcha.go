package handler

import (
	"encoding/json"

	"github.com/Fearless743/flux-panel/go-backend/internal/response"
	"github.com/Fearless743/flux-panel/go-backend/internal/service"
	"github.com/gin-gonic/gin"
)

func (a *App) captchaSvc() *service.CaptchaService {
	return service.NewCaptchaService(a.DB)
}

func (a *App) CaptchaCheck(c *gin.Context) {
	response.OK(c, a.captchaSvc().CheckEnabled())
}

func (a *App) CaptchaGenerate(c *gin.Context) {
	res, err := a.captchaSvc().Generate()
	if err != nil {
		response.Err(c, "生成验证码失败")
		return
	}
	// 兼容 Java CaptchaResponse 风格 + R 风格
	response.OK(c, gin.H{
		"id":              res.ID,
		"captchaId":       res.CaptchaID,
		"captcha":         res,
		"type":            res.Type,
		"captchaImage":    res.CaptchaImage,
		"backgroundImage": res.BackgroundImage,
	})
}

func (a *App) CaptchaVerify(c *gin.Context) {
	// 兼容多种请求体：
	// {id, code} / {captchaId, code} / {id, data: {code|value}}
	raw, err := c.GetRawData()
	if err != nil {
		response.Err(c, "参数错误")
		return
	}
	var m map[string]interface{}
	if err := json.Unmarshal(raw, &m); err != nil {
		response.Err(c, "参数错误")
		return
	}
	id := firstString(m, "id", "captchaId")
	code := firstString(m, "code", "answer", "value")
	if code == "" {
		if data, ok := m["data"]; ok {
			switch v := data.(type) {
			case string:
				code = v
			case map[string]interface{}:
				code = firstString(v, "code", "answer", "value", "data")
			}
		}
	}
	// trackData 简化：直接当答案
	if code == "" {
		code = firstString(m, "trackData")
	}

	ok := a.captchaSvc().Verify(id, code)
	if !ok {
		// 对齐 tianai ApiResponse 失败
		c.JSON(200, gin.H{
			"code":    4001,
			"msg":     "验证码校验失败",
			"success": false,
			"data":    nil,
		})
		return
	}
	c.JSON(200, gin.H{
		"code":    200,
		"msg":     "success",
		"success": true,
		"data":    gin.H{"validToken": id},
	})
}

func firstString(m map[string]interface{}, keys ...string) string {
	for _, k := range keys {
		if v, ok := m[k]; ok {
			if s, ok := v.(string); ok && s != "" {
				return s
			}
		}
	}
	return ""
}
