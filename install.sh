#!/bin/bash
# 下载地址
DOWNLOAD_URL="https://github.com/bqlpfy/forward-panel/releases/download/gost-latest/gost"
INSTALL_DIR="/etc/gost"

# 显示菜单
show_menu() {
  echo "==============================================="
  echo "              管理脚本"
  echo "==============================================="
  echo "请选择操作："
  echo "1. 安装"
  echo "2. 更新"  
  echo "3. 卸载"
  echo "4. 屏蔽协议"
  echo "5. 退出"
  echo "==============================================="
}

# 删除脚本自身
delete_self() {
  echo ""
  echo "🗑️ 操作已完成，正在清理脚本文件..."
  SCRIPT_PATH="$(readlink -f "$0" 2>/dev/null || realpath "$0" 2>/dev/null || echo "$0")"
  sleep 1
  rm -f "$SCRIPT_PATH" && echo "✅ 脚本文件已删除" || echo "❌ 删除脚本文件失败"
}

# 检查并安装 tcpkill
check_and_install_tcpkill() {
  # 检查 tcpkill 是否已安装
  if command -v tcpkill &> /dev/null; then
    return 0
  fi
  
  # 检测操作系统类型
  OS_TYPE=$(uname -s)
  
  # 检查是否需要 sudo
  if [[ $EUID -ne 0 ]]; then
    SUDO_CMD="sudo"
  else
    SUDO_CMD=""
  fi
  
  if [[ "$OS_TYPE" == "Darwin" ]]; then
    if command -v brew &> /dev/null; then
      brew install dsniff &> /dev/null
    fi
    return 0
  fi
  
  # 检测 Linux 发行版并安装对应的包
  if [ -f /etc/os-release ]; then
    . /etc/os-release
    DISTRO=$ID
  elif [ -f /etc/redhat-release ]; then
    DISTRO="rhel"
  elif [ -f /etc/debian_version ]; then
    DISTRO="debian"
  else
    return 0
  fi
  
  case $DISTRO in
    ubuntu|debian)
      $SUDO_CMD apt update &> /dev/null
      $SUDO_CMD apt install -y dsniff &> /dev/null
      ;;
    centos|rhel|fedora)
      if command -v dnf &> /dev/null; then
        $SUDO_CMD dnf install -y dsniff &> /dev/null
      elif command -v yum &> /dev/null; then
        $SUDO_CMD yum install -y dsniff &> /dev/null
      fi
      ;;
    alpine)
      $SUDO_CMD apk add --no-cache dsniff &> /dev/null
      ;;
    arch|manjaro)
      $SUDO_CMD pacman -S --noconfirm dsniff &> /dev/null
      ;;
    opensuse*|sles)
      $SUDO_CMD zypper install -y dsniff &> /dev/null
      ;;
    gentoo)
      $SUDO_CMD emerge --ask=n net-analyzer/dsniff &> /dev/null
      ;;
    void)
      $SUDO_CMD xbps-install -Sy dsniff &> /dev/null
      ;;
  esac
  
  return 0
}

# 检查并安装 iptables
check_and_install_iptables() {
  echo "🔍 检查 iptables..."
  
  # 检查 iptables 是否已安装
  if command -v iptables &> /dev/null; then
    echo "✅ iptables 已安装"
    return 0
  fi
  
  echo "📦 iptables 未安装，正在安装..."
  
  # 检查是否需要 sudo
  if [[ $EUID -ne 0 ]]; then
    SUDO_CMD="sudo"
  else
    SUDO_CMD=""
  fi
  
  # 检测操作系统类型
  OS_TYPE=$(uname -s)
  
  if [[ "$OS_TYPE" == "Darwin" ]]; then
    echo "⚠️  macOS 系统不支持 iptables，请手动安装或使用 pfctl"
    return 1
  fi
  
  # 检测 Linux 发行版并安装对应的包
  if [ -f /etc/os-release ]; then
    . /etc/os-release
    DISTRO=$ID
  elif [ -f /etc/redhat-release ]; then
    DISTRO="rhel"
  elif [ -f /etc/debian_version ]; then
    DISTRO="debian"
  else
    echo "❌ 无法识别的操作系统"
    return 1
  fi
  
  case $DISTRO in
    ubuntu|debian)
      $SUDO_CMD apt update &> /dev/null
      $SUDO_CMD apt install -y iptables &> /dev/null
      ;;
    centos|rhel|fedora)
      if command -v dnf &> /dev/null; then
        $SUDO_CMD dnf install -y iptables &> /dev/null
      elif command -v yum &> /dev/null; then
        $SUDO_CMD yum install -y iptables &> /dev/null
      fi
      ;;
    alpine)
      $SUDO_CMD apk add --no-cache iptables &> /dev/null
      ;;
    arch|manjaro)
      $SUDO_CMD pacman -S --noconfirm iptables &> /dev/null
      ;;
    opensuse*|sles)
      $SUDO_CMD zypper install -y iptables &> /dev/null
      ;;
    gentoo)
      $SUDO_CMD emerge --ask=n net-firewall/iptables &> /dev/null
      ;;
    void)
      $SUDO_CMD xbps-install -Sy iptables &> /dev/null
      ;;
  esac
  
  # 验证安装
  if command -v iptables &> /dev/null; then
    echo "✅ iptables 安装成功"
    return 0
  else
    echo "❌ iptables 安装失败"
    return 1
  fi
}


# 屏蔽协议功能
block_protocol() {
  echo "🛡️ 屏蔽协议功能"
  echo "==============================================="
  
  # 检查 GOST 是否已安装
  if [[ ! -d "$INSTALL_DIR" || ! -f "$INSTALL_DIR/gost" ]]; then
    echo "❌ GOST 服务未安装，请先选择安装选项"
    echo "💡 提示：请先运行选项 1 安装 GOST 服务"
    return 1
  fi
  
  # 检查 GOST 服务是否正在运行
  if ! systemctl is-active --quiet gost; then
    echo "⚠️  GOST 服务未运行，正在启动..."
    systemctl start gost
    sleep 2
    
    if ! systemctl is-active --quiet gost; then
      echo "❌ GOST 服务启动失败，请检查配置"
      echo "💡 提示：请运行 'journalctl -u gost -f' 查看详细错误信息"
      return 1
    fi
  fi
  
  echo "✅ GOST 服务检测通过"
  
  # 检查并安装 iptables
  # if ! check_and_install_iptables; then
  #   echo "❌ iptables 检查失败，无法继续"
  #   return 1
  # fi

    # 验证 IPv4
  is_ipv4() {
    [[ $1 =~ ^([0-9]{1,3}\.){3}[0-9]{1,3}$ ]] && return 0 || return 1
  }

  # 验证 IPv6（简单正则）
  is_ipv6() {
    [[ $1 =~ ^([0-9a-fA-F]{0,4}:){2,7}[0-9a-fA-F]{0,4}$ ]] && return 0 || return 1
  }


  ips=()  # 用于存储所有输入的 IP

  while true; do
    read -p "请输入 IP（多个用逗号分隔，输入 n 结束）: " input
    # 判断是否输入 n
    if [[ "$input" == "n" ]]; then
      break
    fi

    # 使用逗号分割
    IFS=',' read -ra arr <<< "$input"
    for ip in "${arr[@]}"; do
      ip_trimmed=$(echo "$ip" | xargs)  # 去掉首尾空格
      if [[ -z "$ip_trimmed" ]]; then
        continue
      fi

      if is_ipv4 "$ip_trimmed" || is_ipv6 "$ip_trimmed"; then
        ips+=("$ip_trimmed")
      else
        echo "⚠️ 无效 IP: $ip_trimmed"
      fi
    done
  done

  
  # 打印记录的 IP
  for ip in "${ips[@]}"; do
    echo -e "\e[32m$ip\e[0m"
  done

  echo ""
  read -p "是否屏蔽 HTTP? (y/n) [n]: " block_http
  block_http=${block_http:-n}

  read -p "是否屏蔽 TLS? (y/n) [n]: " block_tls
  block_tls=${block_tls:-n}

  read -p "是否屏蔽 SOCKS5? (y/n) [n]: " block_socks5
  block_socks5=${block_socks5:-n}

  echo ""
  echo "🛡️ 屏蔽设置结果:"
  [[ "$block_http" == "y" ]] && echo "  - HTTP 已屏蔽" || echo "  - HTTP 未屏蔽"
  [[ "$block_tls" == "y" ]] && echo "  - TLS 已屏蔽" || echo "  - TLS 未屏蔽"
  [[ "$block_socks5" == "y" ]] && echo "  - SOCKS5 已屏蔽" || echo "  - SOCKS5 未屏蔽"

  # 生成 rules.yaml 文件
  local file="/etc/gost/rules.yaml"
  > "$file"

  # 构造 IP 排除字符串
  ip_expr=""
  for ip in "${ips[@]}"; do
    ip_expr+="ip.src != \"$ip\" && "
  done
  ip_expr=${ip_expr% && }

  # 写入规则
  [[ "$block_http" == "y" ]] && cat >> "$file" <<EOF
- name: block http
  action: block
  log: true
  expr: http != nil && $ip_expr
EOF

  [[ "$block_tls" == "y" ]] && cat >> "$file" <<EOF

- name: block tls
  action: block
  log: true
  expr: tls != nil && $ip_expr
EOF

  [[ "$block_socks5" == "y" ]] && cat >> "$file" <<EOF

- name: block socks
  action: block
  log: true
  expr: socks != nil && $ip_expr
EOF

    echo "📝 已生成 $file"
  
  # 重启 GOST 服务
  echo ""
  echo "🔄 重启 GOST 服务..."
  systemctl restart gost
  echo "5s后检查服务状态"
  sleep 5
  # 检查状态
  echo "检查服务状态..."
  if systemctl is-active --quiet gost; then
    echo "✅ 配置完成，gost服务已重启并正常运行。"
    echo "📁 配置目录: $INSTALL_DIR"
    echo "🔧 服务状态: $(systemctl is-active gost)"
  else
    echo "❌ gost服务启动失败，请执行以下命令查看日志："
    echo "journalctl -u gost -f"
  fi

}

# 获取用户输入的配置参数
get_config_params() {
  if [[ -z "$SERVER_ADDR" || -z "$SECRET" ]]; then
    echo "请输入配置参数："
    
    if [[ -z "$SERVER_ADDR" ]]; then
      read -p "服务器地址: " SERVER_ADDR
    fi
    
    if [[ -z "$SECRET" ]]; then
      read -p "密钥: " SECRET
    fi
    
    if [[ -z "$SERVER_ADDR" || -z "$SECRET" ]]; then
      echo "❌ 参数不完整，操作取消。"
      exit 1
    fi
  fi
}

# 解析命令行参数
while getopts "a:s:" opt; do
  case $opt in
    a) SERVER_ADDR="$OPTARG" ;;
    s) SECRET="$OPTARG" ;;
    *) echo "❌ 无效参数"; exit 1 ;;
  esac
done

# 安装功能
install_gost() {
  echo "🚀 开始安装 GOST..."
  get_config_params
  
  # 询问是否有加速下载地址
  echo ""
  echo "📥 检查下载地址..."
  echo "加速下载地址需提供完整的地址，浏览器打开就能直接下载的那种！！！！！"
  read -p "是否有加速下载地址？(留空使用默认地址): " custom_url
  if [[ -n "$custom_url" ]]; then
    DOWNLOAD_URL="$custom_url"
    echo "✅ 使用自定义下载地址: $DOWNLOAD_URL"
  else
    echo "✅ 使用默认下载地址: $DOWNLOAD_URL"
  fi
  
    # 检查并安装 tcpkill
  check_and_install_tcpkill
  
  # 检查并安装 iptables
  check_and_install_iptables
  
  mkdir -p "$INSTALL_DIR"

  # 停止并禁用已有服务
  if systemctl list-units --full -all | grep -Fq "gost.service"; then
    echo "🔍 检测到已存在的gost服务"
    systemctl stop gost 2>/dev/null && echo "🛑 停止服务"
    systemctl disable gost 2>/dev/null && echo "🚫 禁用自启"
  fi

  # 删除旧文件
  [[ -f "$INSTALL_DIR/gost" ]] && echo "🧹 删除旧文件 gost" && rm -f "$INSTALL_DIR/gost"

  # 下载 gost
  echo "⬇️ 下载 gost 中..."
  curl -L "$DOWNLOAD_URL" -o "$INSTALL_DIR/gost"
  if [[ ! -f "$INSTALL_DIR/gost" || ! -s "$INSTALL_DIR/gost" ]]; then
    echo "❌ 下载失败，请检查网络或下载链接。"
    exit 1
  fi
  chmod +x "$INSTALL_DIR/gost"
  echo "✅ 下载完成"

  # 打印版本
  echo "🔎 gost 版本：$($INSTALL_DIR/gost -V)"

  # 写入 config.json (安装时总是创建新的)
  CONFIG_FILE="$INSTALL_DIR/config.json"
  echo "📄 创建新配置: config.json"
  cat > "$CONFIG_FILE" <<EOF
{
  "addr": "$SERVER_ADDR",
  "secret": "$SECRET"
}
EOF

  # 写入 gost.json
  GOST_CONFIG="$INSTALL_DIR/gost.json"
  if [[ -f "$GOST_CONFIG" ]]; then
    echo "⏭️ 跳过配置文件: gost.json (已存在)"
  else
    echo "📄 创建新配置: gost.json"
    cat > "$GOST_CONFIG" <<EOF
{}
EOF
  fi

  # 加强权限
  chmod 600 "$INSTALL_DIR"/*.json

  # 创建 systemd 服务
  SERVICE_FILE="/etc/systemd/system/gost.service"
  cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=Gost Proxy Service
After=network.target

[Service]
WorkingDirectory=$INSTALL_DIR
ExecStart=$INSTALL_DIR/gost
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOF

  # 启动服务
  systemctl daemon-reload
  systemctl enable gost
  systemctl start gost

  # 检查状态
  echo "🔄 检查服务状态..."
  if systemctl is-active --quiet gost; then
    echo "✅ 安装完成，gost服务已启动并设置为开机启动。"
    echo "📁 配置目录: $INSTALL_DIR"
    echo "🔧 服务状态: $(systemctl is-active gost)"
  else
    echo "❌ gost服务启动失败，请执行以下命令查看日志："
    echo "journalctl -u gost -f"
  fi
}

# 更新功能
update_gost() {
  echo "🔄 开始更新 GOST..."
  
  if [[ ! -d "$INSTALL_DIR" ]]; then
    echo "❌ GOST 未安装，请先选择安装。"
    return 1
  fi
  
  # 询问是否有加速下载地址
  echo ""
  echo "📥 检查下载地址..."
  read -p "是否有加速下载地址？(留空使用默认地址): " custom_url
  if [[ -n "$custom_url" ]]; then
    DOWNLOAD_URL="$custom_url"
    echo "✅ 使用自定义下载地址: $DOWNLOAD_URL"
  else
    echo "✅ 使用默认下载地址: $DOWNLOAD_URL"
  fi
  
  # 检查并安装 tcpkill
  check_and_install_tcpkill
  
  # 检查并安装 iptables
  check_and_install_iptables
  
  # 先下载新版本
  echo "⬇️ 下载最新版本..."
  curl -L "$DOWNLOAD_URL" -o "$INSTALL_DIR/gost.new"
  if [[ ! -f "$INSTALL_DIR/gost.new" || ! -s "$INSTALL_DIR/gost.new" ]]; then
    echo "❌ 下载失败。"
    return 1
  fi

  # 停止服务
  if systemctl list-units --full -all | grep -Fq "gost.service"; then
    echo "🛑 停止 gost 服务..."
    systemctl stop gost
  fi

  # 替换文件
  mv "$INSTALL_DIR/gost.new" "$INSTALL_DIR/gost"
  chmod +x "$INSTALL_DIR/gost"
  
  # 打印版本
  echo "🔎 新版本：$($INSTALL_DIR/gost -V)"

  # 重启服务
  echo "🔄 重启服务..."
  systemctl start gost
  
  echo "✅ 更新完成，服务已重新启动。"
}

# 卸载功能
uninstall_gost() {
  echo "🗑️ 开始卸载 GOST..."
  
  read -p "确认卸载 GOST 吗？此操作将删除所有相关文件 (y/N): " confirm
  if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
    echo "❌ 取消卸载"
    return 0
  fi

  # 停止并禁用服务
  if systemctl list-units --full -all | grep -Fq "gost.service"; then
    echo "🛑 停止并禁用服务..."
    systemctl stop gost 2>/dev/null
    systemctl disable gost 2>/dev/null
  fi

  # 删除服务文件
  if [[ -f "/etc/systemd/system/gost.service" ]]; then
    rm -f "/etc/systemd/system/gost.service"
    echo "🧹 删除服务文件"
  fi

  # 删除安装目录
  if [[ -d "$INSTALL_DIR" ]]; then
    rm -rf "$INSTALL_DIR"
    echo "🧹 删除安装目录: $INSTALL_DIR"
  fi

  # 重载 systemd
  systemctl daemon-reload

  echo "✅ 卸载完成"
}

# 主逻辑
main() {
  # 如果提供了命令行参数，直接执行安装
  if [[ -n "$SERVER_ADDR" && -n "$SECRET" ]]; then
    install_gost
    delete_self
    exit 0
  fi

  # 显示交互式菜单
  while true; do
    show_menu
    read -p "请输入选项 (1-5): " choice
    
    case $choice in
      1)
        install_gost
        delete_self
        exit 0
        ;;
      2)
        update_gost
        delete_self
        exit 0
        ;;
      3)
        uninstall_gost
        delete_self
        exit 0
        ;;
      4)
        block_protocol
        delete_self
        exit 0
        ;;
      5)
        echo "👋 退出脚本"
        delete_self
        exit 0
        ;;
      *)
        echo "❌ 无效选项，请输入 1-5"
        echo ""
        ;;
    esac
  done
}

# 执行主函数
main