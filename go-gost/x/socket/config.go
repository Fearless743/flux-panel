package socket

// saveConfig 已弃用本地 gost.json 落盘。
// 节点配置以面板 DB 为权威源，上线后由面板通过 WS 增量命令重放；
// 运行期命令只更新内存 registry。
func saveConfig() {
	// no-op: do not write gost.json
}
