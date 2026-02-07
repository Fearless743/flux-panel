#!/bin/bash

# 获取系统架构
get_architecture() {
    ARCH=$(uname -m)
    case $ARCH in
        x86_64)
            echo "amd64"
            ;;
        aarch64|arm64)
            echo "arm64"
            ;;
        *)
            echo "amd64"  # 默认使用 amd64
            ;;
    esac
}

# 构建下载地址
build_download_url() {
    local ARCH=$(get_architecture)
    echo "https://raw.githubusercontent.com/Fearless743/flux-panel/refs/heads/beta/go-gost/gost-${ARCH}"
}

# 下载地址
DOWNLOAD_URL=$(build_download_url)
INSTALL_DIR="/etc/flux_agent"
DEFAULT_SERVICE_NAME="flux_agent"
SERVICE_NAME_FILE="$INSTALL_DIR/service_name"
SERVICE_NAME="$DEFAULT_SERVICE_NAME"
COUNTRY=$(curl -s https://ipinfo.io/country)
if [ "$COUNTRY" = "CN" ]; then
    # 拼接 URL
    DOWNLOAD_URL="https://ghfast.top/${DOWNLOAD_URL}"
fi

load_service_name() {
  if [[ -f "$SERVICE_NAME_FILE" ]]; then
    local saved_name
    saved_name=$(tr -d '\r\n' < "$SERVICE_NAME_FILE")
    if [[ -n "$saved_name" ]]; then
      SERVICE_NAME="$saved_name"
      return
    fi
  fi
  SERVICE_NAME="$DEFAULT_SERVICE_NAME"
}

service_exists() {
  local name="$1"
  systemctl list-unit-files --type=service --all | grep -Fq "${name}.service" || \
  systemctl list-units --full -all | grep -Fq "${name}.service"
}

prompt_service_name_if_conflict() {
  SERVICE_NAME="$DEFAULT_SERVICE_NAME"

  if ! service_exists "$SERVICE_NAME"; then
    return
  fi

  echo "⚠️ 检测到已存在服务: ${SERVICE_NAME}.service"
  while true; do
    read -p "请输入新的 service 名称（默认: ${SERVICE_NAME}_1）: " custom_service_name
    custom_service_name=${custom_service_name:-${SERVICE_NAME}_1}

    if [[ ! "$custom_service_name" =~ ^[A-Za-z0-9_.@-]+$ ]]; then
      echo "❌ 服务名仅支持字母、数字、._@-"
      continue
    fi

    if service_exists "$custom_service_name"; then
      echo "❌ 服务 ${custom_service_name}.service 已存在，请换一个名称"
      continue
    fi

    SERVICE_NAME="$custom_service_name"
    break
  done
}



# 显示菜单
show_menu() {
  echo "==============================================="
  echo "              管理脚本"
  echo "==============================================="
  echo "请选择操作："
  echo "1. 安装"
  echo "2. 更新"  
  echo "3. 卸载"
  echo "4. 退出"
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
install_flux_agent() {
  echo "🚀 开始安装 flux_agent..."
  get_config_params

    # 检查并安装 tcpkill
  check_and_install_tcpkill

  load_service_name
  if [[ -f "$SERVICE_NAME_FILE" ]]; then
    echo "🔧 使用已保存的 service 名称: ${SERVICE_NAME}"
  else
    prompt_service_name_if_conflict
    echo "🔧 本次安装 service 名称: ${SERVICE_NAME}"
  fi

  mkdir -p "$INSTALL_DIR"

  echo "$SERVICE_NAME" > "$SERVICE_NAME_FILE"

  # 停止并禁用已有服务
  if service_exists "$SERVICE_NAME"; then
    echo "🔍 检测到已存在的 ${SERVICE_NAME}.service"
    systemctl stop "$SERVICE_NAME" 2>/dev/null && echo "🛑 停止服务"
    systemctl disable "$SERVICE_NAME" 2>/dev/null && echo "🚫 禁用自启"
  fi

  # 删除旧文件
  [[ -f "$INSTALL_DIR/flux_agent" ]] && echo "🧹 删除旧文件 flux_agent" && rm -f "$INSTALL_DIR/flux_agent"

  # 下载 flux_agent
  echo "⬇️ 下载 flux_agent 中..."
  curl -L "$DOWNLOAD_URL" -o "$INSTALL_DIR/flux_agent"
  if [[ ! -f "$INSTALL_DIR/flux_agent" || ! -s "$INSTALL_DIR/flux_agent" ]]; then
    echo "❌ 下载失败，请检查网络或下载链接。"
    exit 1
  fi
  chmod +x "$INSTALL_DIR/flux_agent"
  echo "✅ 下载完成"

  # 打印版本
  echo "🔎 flux_agent 版本：$($INSTALL_DIR/flux_agent -V)"

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
  SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"
  cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=Flux_agent Proxy Service
After=network.target

[Service]
WorkingDirectory=$INSTALL_DIR
ExecStart=$INSTALL_DIR/flux_agent
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOF

  # 启动服务
  systemctl daemon-reload
  systemctl enable "$SERVICE_NAME"
  systemctl start "$SERVICE_NAME"

  # 检查状态
  echo "🔄 检查服务状态..."
  if systemctl is-active --quiet "$SERVICE_NAME"; then
    echo "✅ 安装完成，${SERVICE_NAME} 服务已启动并设置为开机启动。"
    echo "📁 配置目录: $INSTALL_DIR"
    echo "🔧 服务状态: $(systemctl is-active "$SERVICE_NAME")"
  else
    echo "❌ ${SERVICE_NAME} 服务启动失败，请执行以下命令查看日志："
    echo "journalctl -u ${SERVICE_NAME} -f"
  fi
}

# 更新功能
update_flux_agent() {
  load_service_name
  echo "🔄 开始更新 ${SERVICE_NAME}..."
  
  if [[ ! -d "$INSTALL_DIR" ]]; then
    echo "❌ ${SERVICE_NAME} 未安装，请先选择安装。"
    return 1
  fi
  
  echo "📥 使用下载地址: $DOWNLOAD_URL"
  
  # 检查并安装 tcpkill
  check_and_install_tcpkill
  
  # 先下载新版本
  echo "⬇️ 下载最新版本..."
  curl -L "$DOWNLOAD_URL" -o "$INSTALL_DIR/flux_agent.new"
  if [[ ! -f "$INSTALL_DIR/flux_agent.new" || ! -s "$INSTALL_DIR/flux_agent.new" ]]; then
    echo "❌ 下载失败。"
    return 1
  fi

  # 停止服务
  if service_exists "$SERVICE_NAME"; then
    echo "🛑 停止 ${SERVICE_NAME} 服务..."
    systemctl stop "$SERVICE_NAME"
  fi

  # 替换文件
  mv "$INSTALL_DIR/flux_agent.new" "$INSTALL_DIR/flux_agent"
  chmod +x "$INSTALL_DIR/flux_agent"
  
  # 打印版本
  echo "🔎 新版本：$($INSTALL_DIR/flux_agent -V)"

  # 重启服务
  echo "🔄 重启服务..."
  systemctl start "$SERVICE_NAME"

  echo "✅ 更新完成，${SERVICE_NAME} 服务已重新启动。"
}

# 卸载功能
uninstall_flux_agent() {
  load_service_name
  echo "🗑️ 开始卸载 ${SERVICE_NAME}..."

  read -p "确认卸载 ${SERVICE_NAME} 吗？此操作将删除所有相关文件 (y/N): " confirm
  if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
    echo "❌ 取消卸载"
    return 0
  fi

  # 停止并禁用服务
  if service_exists "$SERVICE_NAME"; then
    echo "🛑 停止并禁用服务..."
    systemctl stop "$SERVICE_NAME" 2>/dev/null
    systemctl disable "$SERVICE_NAME" 2>/dev/null
  fi

  # 删除服务文件
  if [[ -f "/etc/systemd/system/${SERVICE_NAME}.service" ]]; then
    rm -f "/etc/systemd/system/${SERVICE_NAME}.service"
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
    install_flux_agent
    delete_self
    exit 0
  fi

  # 显示交互式菜单
  while true; do
    show_menu
    read -p "请输入选项 (1-4): " choice
    
    case $choice in
      1)
        install_flux_agent
        delete_self
        exit 0
        ;;
      2)
        update_flux_agent
        delete_self
        exit 0
        ;;
      3)
        uninstall_flux_agent
        delete_self
        exit 0
        ;;
      4)
        echo "👋 退出脚本"
        delete_self
        exit 0
        ;;
      *)
        echo "❌ 无效选项，请输入 1-4"
        echo ""
        ;;
    esac
  done
}

# 执行主函数
main
