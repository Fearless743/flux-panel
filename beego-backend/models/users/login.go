package users

import (
	"beego-backend/common"
)

func LoginByPassword(data LoginModel) common.ResponseResult {

	return common.ResponseResult{
		Code:    0,
		Success: true,
		Message: "success",
		Data:    nil,
	}
}
