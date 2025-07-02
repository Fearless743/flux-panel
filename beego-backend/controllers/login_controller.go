package controllers

import (
	"beego-backend/common"
	"beego-backend/models/users"
	"encoding/json"
	"fmt"
)

type LoginController struct {
	BaseController
}

// @router /login_by_pwd [post]
func (c *LoginController) LoginByPwd() {
	var Data users.LoginModel
	err := json.Unmarshal(c.Ctx.Input.RequestBody, &Data)
	if err != nil {
		Result := common.ResponseResult{
			Code:    -1,
			Success: false,
			Message: fmt.Sprintf("系统异常：%v", err.Error()),
			Data:    nil,
		}
		c.Data["json"] = &Result
		c.ServeJSON()
		return
	}
	result := users.LoginByPassword(Data)
	c.Data["json"] = &result
	c.ServeJSON()
}
