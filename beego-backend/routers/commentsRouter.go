package routers

import (
	beego "github.com/beego/beego/v2/server/web"
	"github.com/beego/beego/v2/server/web/context/param"
)

func init() {

    beego.GlobalControllerRouter["beego-backend/controllers:LoginController"] = append(beego.GlobalControllerRouter["beego-backend/controllers:LoginController"],
        beego.ControllerComments{
            Method: "LoginByPwd",
            Router: `/login_by_pwd`,
            AllowHTTPMethods: []string{"post"},
            MethodParams: param.Make(),
            Filters: nil,
            Params: nil})

}
