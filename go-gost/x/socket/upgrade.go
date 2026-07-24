package socket

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"time"
)

const (
	defaultAgentDir     = "/etc/flux_agent"
	agentBinName        = "flux_agent"
	agentNewName        = "flux_agent.new"
	agentBakName        = "flux_agent.bak"
	defaultReleaseOwner = "Fearless743"
	defaultReleaseRepo  = "flux-panel"
	upgradeDownloadTO   = 90 * time.Second
	upgradeHealthWait   = 15
)

var upgradeMu sync.Mutex

// upgradeRequest 面板下发的远程升级参数
type upgradeRequest struct {
	URL     string `json:"url"`
	Version string `json:"version"`
	Sha256  string `json:"sha256"`
	Arch    string `json:"arch"`
}

// handleUpgrade 快速校验后异步执行下载/替换/重启，避免阻塞 WS 读循环与面板 10s/30s 超时。
// 成功调度重启前失败会打日志并释放锁；已替换则由脚本健康检查失败时 rollback。
func (w *WebSocketReporter) handleUpgrade(data interface{}) error {
	if !upgradeMu.TryLock() {
		return fmt.Errorf("已有升级任务进行中")
	}

	jsonData, err := json.Marshal(data)
	if err != nil {
		upgradeMu.Unlock()
		return fmt.Errorf("序列化升级参数失败: %v", err)
	}
	var req upgradeRequest
	if err := json.Unmarshal(jsonData, &req); err != nil {
		upgradeMu.Unlock()
		return fmt.Errorf("解析升级参数失败: %v", err)
	}

	dir, binPath, err := resolveAgentPaths()
	if err != nil {
		upgradeMu.Unlock()
		return err
	}

	arch := strings.TrimSpace(req.Arch)
	if arch == "" {
		arch = runtime.GOARCH
	}
	arch = normalizeArch(arch)

	downloadURL, err := resolveUpgradeURL(req.URL, req.Version, arch)
	if err != nil {
		upgradeMu.Unlock()
		return err
	}
	if err := validateUpgradeURL(downloadURL); err != nil {
		upgradeMu.Unlock()
		return err
	}

	sha := strings.TrimSpace(req.Sha256)
	fmt.Printf("⬆️ 接受远程升级任务: url=%s arch=%s\n", downloadURL, arch)

	go func() {
		defer upgradeMu.Unlock()
		if err := performUpgrade(dir, binPath, downloadURL, sha); err != nil {
			fmt.Printf("❌ 远程升级失败: %v\n", err)
			return
		}
		fmt.Println("✅ 升级文件已就绪，重启脚本已调度（失败将自动回滚）")
	}()

	return nil
}

func performUpgrade(dir, binPath, downloadURL, sha256sum string) error {
	newPath := filepath.Join(dir, agentNewName)
	bakPath := filepath.Join(dir, agentBakName)

	if err := downloadFile(downloadURL, newPath, upgradeDownloadTO); err != nil {
		_ = os.Remove(newPath)
		return fmt.Errorf("下载失败: %v", err)
	}
	if err := os.Chmod(newPath, 0o755); err != nil {
		_ = os.Remove(newPath)
		return fmt.Errorf("设置可执行权限失败: %v", err)
	}

	if sha256sum != "" {
		if err := verifySHA256(newPath, sha256sum); err != nil {
			_ = os.Remove(newPath)
			return err
		}
	}

	if err := verifyBinaryRunnable(newPath); err != nil {
		_ = os.Remove(newPath)
		return fmt.Errorf("新二进制自检失败: %v", err)
	}

	if st, err := os.Stat(binPath); err == nil && st.Mode().IsRegular() {
		if err := copyFile(binPath, bakPath); err != nil {
			_ = os.Remove(newPath)
			return fmt.Errorf("备份当前版本失败: %v", err)
		}
		_ = os.Chmod(bakPath, 0o755)
	}

	if err := os.Rename(newPath, binPath); err != nil {
		if err2 := copyFile(newPath, binPath); err2 != nil {
			_ = os.Remove(newPath)
			return fmt.Errorf("替换二进制失败: %v / %v", err, err2)
		}
		_ = os.Remove(newPath)
		_ = os.Chmod(binPath, 0o755)
	} else {
		_ = os.Chmod(binPath, 0o755)
	}

	_ = os.Remove(filepath.Join(dir, "gost.json"))

	if err := scheduleRestartWithRollback(dir, binPath, bakPath); err != nil {
		if _, e := os.Stat(bakPath); e == nil {
			_ = copyFile(bakPath, binPath)
			_ = os.Chmod(binPath, 0o755)
		}
		return fmt.Errorf("调度重启失败: %v", err)
	}
	return nil
}

func resolveAgentPaths() (dir, bin string, err error) {
	if exe, e := os.Executable(); e == nil {
		if real, e2 := filepath.EvalSymlinks(exe); e2 == nil {
			exe = real
		}
		return filepath.Dir(exe), exe, nil
	}
	dir = defaultAgentDir
	bin = filepath.Join(dir, agentBinName)
	if st, e := os.Stat(dir); e != nil || !st.IsDir() {
		wd, _ := os.Getwd()
		if wd != "" {
			return wd, filepath.Join(wd, agentBinName), nil
		}
		return "", "", fmt.Errorf("无法确定安装目录")
	}
	return dir, bin, nil
}

func normalizeArch(arch string) string {
	switch strings.ToLower(arch) {
	case "x86_64", "amd64":
		return "amd64"
	case "aarch64", "arm64":
		return "arm64"
	default:
		return arch
	}
}

func resolveUpgradeURL(rawURL, version, arch string) (string, error) {
	rawURL = strings.TrimSpace(rawURL)
	if rawURL != "" {
		return rawURL, nil
	}
	version = strings.TrimSpace(version)
	if version == "" || strings.EqualFold(version, "latest") {
		return fmt.Sprintf(
			"https://github.com/%s/%s/releases/latest/download/gost-%s",
			defaultReleaseOwner, defaultReleaseRepo, arch,
		), nil
	}
	return fmt.Sprintf(
		"https://github.com/%s/%s/releases/download/%s/gost-%s",
		defaultReleaseOwner, defaultReleaseRepo, version, arch,
	), nil
}

func validateUpgradeURL(raw string) error {
	u, err := url.Parse(raw)
	if err != nil {
		return fmt.Errorf("无效的升级地址: %v", err)
	}
	if u.Scheme != "https" {
		return fmt.Errorf("升级地址仅允许 https")
	}
	host := strings.ToLower(u.Host)
	path := u.Path
	needle := strings.ToLower(fmt.Sprintf("%s/%s/releases/", defaultReleaseOwner, defaultReleaseRepo))

	if host == "github.com" {
		prefix := fmt.Sprintf("/%s/%s/releases/", defaultReleaseOwner, defaultReleaseRepo)
		if strings.HasPrefix(path, prefix) {
			return nil
		}
		return fmt.Errorf("升级地址不在白名单内")
	}
	if host == "ghfast.top" || host == "ghproxy.net" || host == "mirror.ghproxy.com" {
		if strings.Contains(strings.ToLower(raw), needle) {
			return nil
		}
		return fmt.Errorf("加速地址未指向官方 Release")
	}
	return fmt.Errorf("升级地址域名不在白名单: %s", host)
}

func downloadFile(rawURL, dest string, timeout time.Duration) error {
	client := &http.Client{Timeout: timeout}
	req, err := http.NewRequest(http.MethodGet, rawURL, nil)
	if err != nil {
		return err
	}
	req.Header.Set("User-Agent", "flux-agent-upgrade")
	resp, err := client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("HTTP %d", resp.StatusCode)
	}

	tmp := dest + ".part"
	_ = os.Remove(tmp)
	f, err := os.OpenFile(tmp, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0o755)
	if err != nil {
		return err
	}
	written, err := io.Copy(f, resp.Body)
	closeErr := f.Close()
	if err != nil {
		_ = os.Remove(tmp)
		return err
	}
	if closeErr != nil {
		_ = os.Remove(tmp)
		return closeErr
	}
	if written < 1024 {
		_ = os.Remove(tmp)
		return fmt.Errorf("下载文件过小 (%d bytes)，可能不是有效二进制", written)
	}
	if err := os.Rename(tmp, dest); err != nil {
		_ = os.Remove(tmp)
		return err
	}
	return nil
}

func verifySHA256(path, expect string) error {
	f, err := os.Open(path)
	if err != nil {
		return err
	}
	defer f.Close()
	h := sha256.New()
	if _, err := io.Copy(h, f); err != nil {
		return err
	}
	got := hex.EncodeToString(h.Sum(nil))
	if !strings.EqualFold(got, strings.TrimSpace(expect)) {
		return fmt.Errorf("sha256 校验失败: expect=%s got=%s", expect, got)
	}
	return nil
}

func verifyBinaryRunnable(path string) error {
	cmd := exec.Command(path, "-V")
	cmd.Env = os.Environ()
	var out strings.Builder
	cmd.Stdout = &out
	cmd.Stderr = &out
	done := make(chan error, 1)
	go func() { done <- cmd.Run() }()
	select {
	case err := <-done:
		if err != nil {
			return fmt.Errorf("%v (%s)", err, strings.TrimSpace(out.String()))
		}
		return nil
	case <-time.After(5 * time.Second):
		if cmd.Process != nil {
			_ = cmd.Process.Kill()
		}
		return fmt.Errorf("执行 -V 超时")
	}
}

func copyFile(src, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()
	tmp := dst + ".tmp"
	out, err := os.OpenFile(tmp, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0o755)
	if err != nil {
		return err
	}
	_, copyErr := io.Copy(out, in)
	closeErr := out.Close()
	if copyErr != nil {
		_ = os.Remove(tmp)
		return copyErr
	}
	if closeErr != nil {
		_ = os.Remove(tmp)
		return closeErr
	}
	return os.Rename(tmp, dst)
}

func scheduleRestartWithRollback(dir, binPath, bakPath string) error {
	script := fmt.Sprintf(`#!/bin/bash
set -u
BIN=%q
BAK=%q
LOG=%q
exec >>"$LOG" 2>&1
echo "$(date -Iseconds) upgrade-apply start"
sleep 1
if command -v systemctl >/dev/null 2>&1; then
  systemctl restart flux_agent || true
fi
ok=0
for i in $(seq 1 %d); do
  if command -v systemctl >/dev/null 2>&1 && systemctl is-active --quiet flux_agent; then
    ok=1
    break
  fi
  if pgrep -f "$BIN" >/dev/null 2>&1; then
    ok=1
    break
  fi
  sleep 1
done
if [[ "$ok" -eq 1 ]]; then
  echo "$(date -Iseconds) upgrade success"
  rm -f %q
  exit 0
fi
echo "$(date -Iseconds) health check failed, rollback"
if [[ -f "$BAK" ]]; then
  cp -a "$BAK" "$BIN"
  chmod +x "$BIN"
  if command -v systemctl >/dev/null 2>&1; then
    systemctl restart flux_agent || true
    sleep 2
    if systemctl is-active --quiet flux_agent; then
      echo "$(date -Iseconds) rollback success"
      exit 1
    fi
  fi
  echo "$(date -Iseconds) rollback attempted"
  exit 1
fi
echo "$(date -Iseconds) no bak, cannot rollback"
exit 1
`, binPath, bakPath, filepath.Join(dir, "upgrade-apply.log"), upgradeHealthWait, filepath.Join(dir, agentNewName))

	scriptPath := filepath.Join(dir, "upgrade-apply.sh")
	if err := os.WriteFile(scriptPath, []byte(script), 0o700); err != nil {
		return err
	}

	if path, err := exec.LookPath("systemd-run"); err == nil {
		cmd := exec.Command(path, "--no-block", "/bin/bash", scriptPath)
		if err := cmd.Start(); err == nil {
			_ = cmd.Process.Release()
			return nil
		}
	}

	cmd := exec.Command("/bin/bash", "-c", "nohup /bin/bash "+shellQuote(scriptPath)+" >/dev/null 2>&1 &")
	if err := cmd.Start(); err != nil {
		return err
	}
	_ = cmd.Process.Release()
	return nil
}

func shellQuote(s string) string {
	return `'` + strings.ReplaceAll(s, `'`, `'\''`) + `'`
}
