#!/bin/bash
set -e

# 解决 macOS 下 tr 可能出现的非法字节序列问题
export LANG=en_US.UTF-8
export LC_ALL=C

# 全局下载地址配置（配置文件从仓库分支获取，与安装脚本同源）
BRANCH="beta"
DOCKER_COMPOSEV4_URL="https://raw.githubusercontent.com/Fearless743/flux-panel/refs/heads/${BRANCH}/docker-compose-v4.yml"
DOCKER_COMPOSEV6_URL="https://raw.githubusercontent.com/Fearless743/flux-panel/refs/heads/${BRANCH}/docker-compose-v6.yml"

COUNTRY=$(curl -s https://ipinfo.io/country)
if [ "$COUNTRY" = "CN" ]; then
    DOCKER_COMPOSEV4_URL="https://ghfast.top/${DOCKER_COMPOSEV4_URL}"
    DOCKER_COMPOSEV6_URL="https://ghfast.top/${DOCKER_COMPOSEV6_URL}"
fi

get_docker_compose_url() {
  if check_ipv6_support > /dev/null 2>&1; then
    echo "$DOCKER_COMPOSEV6_URL"
  else
    echo "$DOCKER_COMPOSEV4_URL"
  fi
}

check_docker() {
  if command -v docker-compose &> /dev/null; then
    DOCKER_CMD="docker-compose"
  elif command -v docker &> /dev/null; then
    if docker compose version &> /dev/null; then
      DOCKER_CMD="docker compose"
    else
      echo "错误：检测到 docker，但不支持 'docker compose' 命令。请安装 docker-compose 或更新 docker 版本。"
      exit 1
    fi
  else
    echo "错误：未检测到 docker 或 docker-compose 命令。请先安装 Docker。"
    exit 1
  fi
  echo "检测到 Docker 命令：$DOCKER_CMD"
}

check_ipv6_support() {
  echo "🔍 检测 IPv6 支持..."

  if ip -6 addr show | grep -v "scope link" | grep -q "inet6"; then
    echo "✅ 检测到系统支持 IPv6"
    return 0
  elif ifconfig 2>/dev/null | grep -v "fe80:" | grep -q "inet6"; then
    echo "✅ 检测到系统支持 IPv6"
    return 0
  else
    echo "⚠️ 未检测到 IPv6 支持"
    return 1
  fi
}

configure_docker_ipv6() {
  echo "🔧 配置 Docker IPv6 支持..."

  OS_TYPE=$(uname -s)

  if [[ "$OS_TYPE" == "Darwin" ]]; then
    echo "✅ macOS Docker Desktop 默认支持 IPv6"
    return 0
  fi

  DOCKER_CONFIG="/etc/docker/daemon.json"

  if [[ $EUID -ne 0 ]]; then
    SUDO_CMD="sudo"
  else
    SUDO_CMD=""
  fi

  if [ -f "$DOCKER_CONFIG" ]; then
    if grep -q '"ipv6"' "$DOCKER_CONFIG"; then
      echo "✅ Docker 已配置 IPv6 支持"
    else
      echo "📝 更新 Docker 配置以启用 IPv6..."
      $SUDO_CMD cp "$DOCKER_CONFIG" "${DOCKER_CONFIG}.backup"

      if command -v jq &> /dev/null; then
        $SUDO_CMD jq '. + {"ipv6": true, "fixed-cidr-v6": "fd00::/80"}' "$DOCKER_CONFIG" > /tmp/daemon.json && $SUDO_CMD mv /tmp/daemon.json "$DOCKER_CONFIG"
      else
        $SUDO_CMD sed -i 's/^{$/{\n  "ipv6": true,\n  "fixed-cidr-v6": "fd00::\/80",/' "$DOCKER_CONFIG"
      fi

      echo "🔄 重启 Docker 服务..."
      if command -v systemctl &> /dev/null; then
        $SUDO_CMD systemctl restart docker
      elif command -v service &> /dev/null; then
        $SUDO_CMD service docker restart
      else
        echo "⚠️ 请手动重启 Docker 服务"
      fi
      sleep 5
    fi
  else
    echo "📝 创建 Docker 配置文件..."
    $SUDO_CMD mkdir -p /etc/docker
    echo '{
  "ipv6": true,
  "fixed-cidr-v6": "fd00::/80"
}' | $SUDO_CMD tee "$DOCKER_CONFIG" > /dev/null

    echo "🔄 重启 Docker 服务..."
    if command -v systemctl &> /dev/null; then
      $SUDO_CMD systemctl restart docker
    elif command -v service &> /dev/null; then
      $SUDO_CMD service docker restart
    else
      echo "⚠️ 请手动重启 Docker 服务"
    fi
    sleep 5
  fi
}

show_menu() {
  echo "==============================================="
  echo "          面板管理脚本（单镜像）"
  echo "==============================================="
  echo "请选择操作："
  echo "1. 安装面板"
  echo "2. 更新面板"
  echo "3. 卸载面板"
  echo "4. 退出"
  echo "==============================================="
}

generate_random() {
  LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | head -c16
}

delete_self() {
  echo ""
  echo "🗑️ 操作已完成，正在清理脚本文件..."
  SCRIPT_PATH="$(readlink -f "$0" 2>/dev/null || realpath "$0" 2>/dev/null || echo "$0")"
  sleep 1
  rm -f "$SCRIPT_PATH" && echo "✅ 脚本文件已删除" || echo "❌ 删除脚本文件失败"
}

get_config_params() {
  echo "🔧 请输入配置参数："
  echo "（前后端已聚合为单一镜像，仅需一个访问端口）"

  read -p "面板端口（默认 6366）: " PANEL_PORT
  PANEL_PORT=${PANEL_PORT:-6366}

  # 兼容旧 .env：若用户仍习惯分别填，可忽略 BACKEND
  JWT_SECRET=$(generate_random)
}

install_panel() {
  echo "🚀 开始安装面板（flux-panel 单镜像）..."
  check_docker
  get_config_params

  echo "🔽 下载必要文件..."
  DOCKER_COMPOSE_URL=$(get_docker_compose_url)
  echo "📡 选择配置文件：$(basename "$DOCKER_COMPOSE_URL")"
  curl -L -o docker-compose.yml "$DOCKER_COMPOSE_URL"
  echo "✅ 文件准备完成"

  if check_ipv6_support; then
    echo "🚀 系统支持 IPv6，自动启用 IPv6 配置..."
    configure_docker_ipv6
  fi

  cat > .env <<EOF
JWT_SECRET=$JWT_SECRET
PANEL_PORT=$PANEL_PORT
# 兼容旧变量名（compose 已不使用）
FRONTEND_PORT=$PANEL_PORT
BACKEND_PORT=$PANEL_PORT
EOF

  echo "🚀 启动 docker 服务..."
  $DOCKER_CMD up -d

  echo "🎉 部署完成"
  echo "🌐 访问地址: http://服务器IP:$PANEL_PORT"
  echo "📡 节点连接面板请使用同一地址/端口（/system-info、/flow 同源）"
  echo "📖 部署完成后请阅读使用文档"
  echo "📚 文档地址: https://tes.cc/guide.html"
  echo "💡 默认管理员账号: admin_user / admin_user"
  echo "⚠️  登录后请立即修改默认密码！"
  echo "📦 镜像: ghcr.io/fearless743/flux-panel:latest"
}

update_panel() {
  echo "🔄 开始更新面板..."
  check_docker

  echo "🔽 下载最新配置文件..."
  DOCKER_COMPOSE_URL=$(get_docker_compose_url)
  echo "📡 选择配置文件：$(basename "$DOCKER_COMPOSE_URL")"
  curl -L -o docker-compose.yml "$DOCKER_COMPOSE_URL"
  echo "✅ 下载完成"

  # 迁移旧 .env：双端口 → PANEL_PORT
  if [[ -f .env ]]; then
    if ! grep -q '^PANEL_PORT=' .env 2>/dev/null; then
      OLD_FRONT=$(grep -E '^FRONTEND_PORT=' .env 2>/dev/null | cut -d= -f2- || true)
      if [[ -n "$OLD_FRONT" ]]; then
        echo "PANEL_PORT=$OLD_FRONT" >> .env
        echo "📝 已从 FRONTEND_PORT 迁移 PANEL_PORT=$OLD_FRONT"
      else
        echo "PANEL_PORT=6366" >> .env
      fi
    fi
  fi

  if check_ipv6_support; then
    echo "🚀 系统支持 IPv6，自动启用 IPv6 配置..."
    configure_docker_ipv6
  fi

  # 兼容旧容器名
  PANEL_CONTAINER="flux-panel"
  docker stop -t 30 "$PANEL_CONTAINER" 2>/dev/null || true
  docker stop -t 30 springboot-backend 2>/dev/null || true
  docker stop -t 10 vite-frontend 2>/dev/null || true

  echo "⏳ 等待数据同步..."
  sleep 5

  $DOCKER_CMD down

  echo "⬇️ 拉取最新镜像（flux-panel 一体镜像）..."
  $DOCKER_CMD pull

  echo "🚀 启动更新后的服务..."
  $DOCKER_CMD up -d

  echo "⏳ 等待服务启动..."
  echo "🔍 检查面板服务状态..."
  for i in {1..90}; do
    if docker ps --format "{{.Names}}" | grep -q "^${PANEL_CONTAINER}$"; then
      PANEL_HEALTH=$(docker inspect -f '{{.State.Health.Status}}' "$PANEL_CONTAINER" 2>/dev/null || echo "unknown")
      if [[ "$PANEL_HEALTH" == "healthy" ]]; then
        echo "✅ 面板服务健康检查通过"
        break
      elif [[ "$PANEL_HEALTH" == "starting" ]]; then
        :
      elif [[ "$PANEL_HEALTH" == "unhealthy" ]]; then
        echo "⚠️ 健康状态：$PANEL_HEALTH"
      fi
    else
      echo "⚠️ 面板容器未找到或未运行"
      PANEL_HEALTH="not_running"
    fi
    if [ $i -eq 90 ]; then
      echo "❌ 面板服务启动超时（90秒）"
      echo "🔍 当前状态：$(docker inspect -f '{{.State.Health.Status}}' "$PANEL_CONTAINER" 2>/dev/null || echo '容器不存在')"
      echo "🛑 更新终止"
      return 1
    fi
    if [ $((i % 15)) -eq 1 ]; then
      echo "⏳ 等待面板服务启动... ($i/90) 状态：${PANEL_HEALTH:-unknown}"
    fi
    sleep 1
  done

  echo "✅ 更新完成（镜像：flux-panel）"
}

uninstall_panel() {
  echo "🗑️ 开始卸载面板..."
  check_docker

  if [[ ! -f "docker-compose.yml" ]]; then
    echo "⚠️ 未找到 docker-compose.yml 文件，正在下载以完成卸载..."
    DOCKER_COMPOSE_URL=$(get_docker_compose_url)
    echo "📡 选择配置文件：$(basename "$DOCKER_COMPOSE_URL")"
    curl -L -o docker-compose.yml "$DOCKER_COMPOSE_URL"
    echo "✅ docker-compose.yml 下载完成"
  fi

  read -p "确认卸载面板吗？此操作将停止并删除所有容器和数据 (y/N): " confirm
  if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
    echo "❌ 取消卸载"
    return 0
  fi

  echo "🛑 停止并删除容器、镜像、卷..."
  $DOCKER_CMD down --rmi all --volumes --remove-orphans
  echo "🧹 删除配置文件..."
  rm -f docker-compose.yml .env
  echo "✅ 卸载完成"
}

main() {
  while true; do
    show_menu
    read -p "请输入选项 (1-4): " choice

    case $choice in
      1)
        install_panel
        delete_self
        exit 0
        ;;
      2)
        update_panel
        delete_self
        exit 0
        ;;
      3)
        uninstall_panel
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

main
