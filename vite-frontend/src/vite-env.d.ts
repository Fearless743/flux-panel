/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** 后端 API 基址；生产一体镜像为空，走同源 */
  readonly VITE_API_BASE?: string
  /** 管理后台显示版本（CI 注入 git tag） */
  readonly VITE_APP_VERSION?: string
  /** 移动端/WebView 显示版本 */
  readonly VITE_MOBILE_APP_VERSION?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
