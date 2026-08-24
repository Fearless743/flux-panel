## FEAR-76 安全审计报告

### 漏洞信息
- **漏洞 ID**: GHSA-fx2h-pf6j-xcff
- **影响包**: vite@5.4.21
- **脆弱版本**: <=6.4.2
- **CVSS**: 7.5 (AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:N/A:N)
- **评级**: P1（高危）

### 审计发现
flux-panel 仓库的 `vite-frontend/package.json` 中 vite 版本为 `5.4.21`，属于受影响版本。

### 漏洞详情
Vite 的 `server.fs.deny` 功能在 Windows 备用数据流（ADS）路径下存在绕过漏洞：
- 攻击者可构造请求如 `/.env::$DATA?raw` 读取被禁止的文件
- 同理，8.3 短文件名也可绕过访问限制

### 修复方案
将 vite 版本从 `5.4.21` 升级到 `^6.4.3`

