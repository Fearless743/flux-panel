package handler

import (
	"github.com/Fearless743/flux-panel/go-backend/internal/response"
	"github.com/Fearless743/flux-panel/go-backend/internal/service"
	"github.com/gin-gonic/gin"
)

func (a *App) backupSvc() *service.BackupService {
	return service.NewBackupService(a.DB)
}

// BackupExport 导出全部数据（管理员）
func (a *App) BackupExport(c *gin.Context) {
	data, err := a.backupSvc().Export()
	if err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, data)
}

// BackupImport 全量覆盖导入（管理员，危险操作，前端需二次确认）
func (a *App) BackupImport(c *gin.Context) {
	var data service.BackupData
	if err := c.ShouldBindJSON(&data); err != nil {
		response.Err(c, "参数错误：不是有效的备份文件")
		return
	}
	if err := a.backupSvc().Import(&data); err != nil {
		response.Err(c, err.Error())
		return
	}
	response.OK(c, nil)
}
