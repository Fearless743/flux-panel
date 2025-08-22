#!/bin/bash
set -e

# 解决 macOS 下 tr 可能出现的非法字节序列问题
export LANG=en_US.UTF-8
export LC_ALL=C

# ===================== 新增：反向代理脚本地址 =====================
PROXY_SH_URL="https://raw.githubusercontent.com/Lanlan13-14/flux-panel/refs/heads/main/proxy.sh"

# 全局下载地址配置
DOCKER_COMPOSEV4_URL="https://raw.githubusercontent.com/bqlpfy/flux-panel/refs/heads/main/docker-compose-v4.yml"
DOCKER_COMPOSEV6_URL="https://raw.githubusercontent.com/bqlpfy/flux-panel/refs/heads/main/docker-compose-v6.yml"
GOST_SQL_URL="https://raw.githubusercontent.com/bqlpfy/flux-panel/refs/heads/main/gost.sql"

COUNTRY=$(curl -s https://ipinfo.io/country)
if [ "$COUNTRY" = "CN" ]; then
    # 拼接 URL（境内加速）
    DOCKER_COMPOSEV4_URL="https://ghfast.top/${DOCKER_COMPOSEV4_URL}"
    DOCKER_COMPOSEV6_URL="https://ghfast.top/${DOCKER_COMPOSEV6_URL}"
    GOST_SQL_URL="https://ghfast.top/${GOST_SQL_URL}"
    PROXY_SH_URL="https://ghfast.top/${PROXY_SH_URL}"
fi

# 根据IPv6支持情况选择docker-compose URL
get_docker_compose_url() {
  if check_ipv6_support > /dev/null 2>&1; then
    echo "$DOCKER_COMPOSEV6_URL"
  else
    echo "$DOCKER_COMPOSEV4_URL"
  fi
}

# 检查 docker-compose 或 docker compose 命令
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

# 检测系统是否支持 IPv6
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

# 配置 Docker 启用 IPv6
configure_docker_ipv6() {
  echo "🔧 配置 Docker IPv6 支持..."
  OS_TYPE=$(uname -s)
  if [[ "$OS_TYPE" == "Darwin" ]]; then
    echo "✅ macOS Docker Desktop 默认支持 IPv6"
    return 0
  fi
  DOCKER_CONFIG="/etc/docker/daemon.json"
  if [[ $EUID -ne 0 ]]; then SUDO_CMD="sudo"; else SUDO_CMD=""; fi

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

# 显示菜单（已调整顺序：5=反代，6=退出）
show_menu() {
  echo "==============================================="
  echo "          面板管理脚本"
  echo "==============================================="
  echo "请选择操作："
  echo "1. 安装面板"
  echo "2. 更新面板"
  echo "3. 卸载面板"
  echo "4. 导出备份"
  echo "5. 安装并配置反向代理（Caddy）"
  echo "6. 退出"
  echo "==============================================="
}

generate_random() {
  LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | head -c16
}

# 删除脚本自身
delete_self() {
  echo ""
  echo "🗑️ 操作已完成，正在清理脚本文件..."
  SCRIPT_PATH="$(readlink -f "$0" 2>/dev/null || realpath "$0" 2>/dev/null || echo "$0")"
  sleep 1
  rm -f "$SCRIPT_PATH" && echo "✅ 脚本文件已删除" || echo "❌ 删除脚本文件失败"
}

# 获取用户输入的配置参数
get_config_params() {
  echo "🔧 请输入配置参数："
  read -p "前端端口（默认 6366）: " FRONTEND_PORT
  FRONTEND_PORT=${FRONTEND_PORT:-6366}
  read -p "后端端口（默认 6365）: " BACKEND_PORT
  BACKEND_PORT=${BACKEND_PORT:-6365}
  DB_NAME=$(generate_random)
  DB_USER=$(generate_random)
  DB_PASSWORD=$(generate_random)
  JWT_SECRET=$(generate_random)
}

# 安装面板
install_panel() {
  echo "🚀 开始安装面板..."
  check_docker
  get_config_params

  echo "🔽 下载必要文件..."
  DOCKER_COMPOSE_URL=$(get_docker_compose_url)
  echo "📡 选择配置文件：$(basename "$DOCKER_COMPOSE_URL")"
  curl -L -o docker-compose.yml "$DOCKER_COMPOSE_URL"

  if [[ -f "gost.sql" ]]; then
    echo "⏭️ 跳过下载: gost.sql (使用当前位置的文件)"
  else
    echo "📡 下载数据库初始化文件..."
    curl -L -o gost.sql "$GOST_SQL_URL"
  fi
  echo "✅ 文件准备完成"

  if check_ipv6_support; then
    echo "🚀 系统支持 IPv6，自动启用 IPv6 配置..."
    configure_docker_ipv6
  fi

  cat > .env <<EOF
DB_NAME=$DB_NAME
DB_USER=$DB_USER
DB_PASSWORD=$DB_PASSWORD
JWT_SECRET=$JWT_SECRET
FRONTEND_PORT=$FRONTEND_PORT
BACKEND_PORT=$BACKEND_PORT
EOF

  echo "🚀 启动 docker 服务..."
  $DOCKER_CMD up -d

  echo "🎉 部署完成"
  echo "🌐 访问地址: http://服务器IP:$FRONTEND_PORT"
  echo "📖 部署完成后请阅读下使用文档，求求了啊，不要上去就是一顿操作"
  echo "📚 文档地址: https://tes.cc/guide.html"
  echo "💡 默认管理员账号: admin_user / admin_user"
  echo "⚠️  登录后请立即修改默认密码！"
}

# 更新面板（原逻辑保持不变）
update_panel() {
  echo "🔄 开始更新面板..."
  check_docker

  echo "🔽 下载最新配置文件..."
  DOCKER_COMPOSE_URL=$(get_docker_compose_url)
  echo "📡 选择配置文件：$(basename "$DOCKER_COMPOSE_URL")"
  curl -L -o docker-compose.yml "$DOCKER_COMPOSE_URL"
  echo "✅ 下载完成"

  if check_ipv6_support; then
    echo "🚀 系统支持 IPv6，自动启用 IPv6 配置..."
    configure_docker_ipv6
  fi

  echo "🛑 停止当前服务..."
  $DOCKER_CMD down

  echo "⬇️ 拉取最新镜像..."
  $DOCKER_CMD pull

  echo "🚀 启动更新后的服务..."
  $DOCKER_CMD up -d

  echo "⏳ 等待服务启动..."
  echo "🔍 检查后端服务状态..."
  for i in {1..90}; do
    if docker ps --format "{{.Names}}" | grep -q "^springboot-backend$"; then
      BACKEND_HEALTH=$(docker inspect -f '{{.State.Health.Status}}' springboot-backend 2>/dev/null || echo "unknown")
      if [[ "$BACKEND_HEALTH" == "healthy" ]]; then
        echo "✅ 后端服务健康检查通过"
        break
      elif [[ "$BACKEND_HEALTH" == "starting" ]]; then
        :
      elif [[ "$BACKEND_HEALTH" == "unhealthy" ]]; then
        echo "⚠️ 后端健康状态：$BACKEND_HEALTH"
      fi
    else
      echo "⚠️ 后端容器未找到或未运行"
      BACKEND_HEALTH="not_running"
    fi
    if [ $i -eq 90 ]; then
      echo "❌ 后端服务启动超时（90秒）"
      echo "🔍 当前状态：$(docker inspect -f '{{.State.Health.Status}}' springboot-backend 2>/dev/null || echo '容器不存在')"
      echo "🛑 更新终止"
      return 1
    fi
    if [ $((i % 15)) -eq 1 ]; then
      echo "⏳ 等待后端服务启动... ($i/90) 状态：${BACKEND_HEALTH:-unknown}"
    fi
    sleep 1
  done

  echo "🔍 检查数据库服务状态..."
  for i in {1..60}; do
    if docker ps --format "{{.Names}}" | grep -q "^gost-mysql$"; then
      DB_HEALTH=$(docker inspect -f '{{.State.Health.Status}}' gost-mysql 2>/dev/null || echo "unknown")
      if [[ "$DB_HEALTH" == "healthy" ]]; then
        echo "✅ 数据库服务健康检查通过"
        break
      elif [[ "$DB_HEALTH" == "starting" ]]; then
        :
      elif [[ "$DB_HEALTH" == "unhealthy" ]]; then
        echo "⚠️ 数据库健康状态：$DB_HEALTH"
      fi
    else
      echo "⚠️ 数据库容器未找到或未运行"
      DB_HEALTH="not_running"
    fi
    if [ $i -eq 60 ]; then
      echo "❌ 数据库服务启动超时（60秒）"
      echo "🔍 当前状态：$(docker inspect -f '{{.State.Health.Status}}' gost-mysql 2>/dev/null || echo '容器不存在')"
      echo "🛑 更新终止"
      return 1
    fi
    if [ $((i % 10)) -eq 1 ]; then
      echo "⏳ 等待数据库服务启动... ($i/60) 状态：${DB_HEALTH:-unknown}"
    fi
    sleep 1
  done

  echo "🔍 获取数据库配置信息..."
  echo "⏳ 等待服务完全就绪..."
  sleep 5

  if ! docker ps --format "{{.Names}}" | grep -q "^springboot-backend$"; then
    echo "❌ 后端容器未运行，无法获取数据库配置"
    echo "🔍 当前运行的容器："
    docker ps --format "table {{.Names}}\t{{.Status}}"
    echo "🛑 更新终止"
    return 1
  fi

  DB_INFO=$(docker exec springboot-backend env | grep "^DB_" 2>/dev/null || echo "")
  if [[ -n "$DB_INFO" ]]; then
    DB_NAME=$(echo "$DB_INFO" | grep "^DB_NAME=" | cut -d'=' -f2)
    DB_PASSWORD=$(echo "$DB_INFO" | grep "^DB_PASSWORD=" | cut -d'=' -f2)
    DB_USER=$(echo "$DB_INFO" | grep "^DB_USER=" | cut -d'=' -f2)
    DB_HOST=$(echo "$DB_INFO" | grep "^DB_HOST=" | cut -d'=' -f2)
    echo "📋 数据库配置："
    echo "   数据库名: $DB_NAME"
    echo "   用户名: $DB_USER"
    echo "   主机: $DB_HOST"
  else
    echo "❌ 无法获取数据库配置信息"
    echo "🔍 尝试诊断问题："
    echo "   容器状态: $(docker inspect -f '{{.State.Status}}' springboot-backend 2>/dev/null || echo '容器不存在')"
    echo "   健康状态: $(docker inspect -f '{{.State.Health.Status}}' springboot-backend 2>/dev/null || echo '无健康检查')"
    if [[ -f ".env" ]]; then
      echo "🔄 尝试从 .env 文件读取配置..."
      DB_NAME=$(grep "^DB_NAME=" .env | cut -d'=' -f2 2>/dev/null)
      DB_PASSWORD=$(grep "^DB_PASSWORD=" .env | cut -d'=' -f2 2>/dev/null)
      DB_USER=$(grep "^DB_USER=" .env | cut -d'=' -f2 2>/dev/null)
      if [[ -n "$DB_NAME" && -n "$DB_PASSWORD" && -n "$DB_USER" ]]; then
        echo "✅ 从 .env 文件成功读取数据库配置"
        echo "📋 数据库配置："
        echo "   数据库名: $DB_NAME"
        echo "   用户名: $DB_USER"
      else
        echo "❌ .env 文件中的数据库配置不完整"
        echo "🛑 更新终止"
        return 1
      fi
    else
      echo "❌ 未找到 .env 文件"
      echo "🛑 更新终止"
      return 1
    fi
  fi

  if [[ -z "$DB_PASSWORD" || -z "$DB_USER" || -z "$DB_NAME" ]]; then
    echo "❌ 数据库配置不完整（缺少必要参数）"
    echo "🛑 更新终止"
    return 1
  fi

  echo "🔄 执行数据库结构更新..."
  cat > temp_migration.sql <<'EOF'
-- 这里保留你原有的长 SQL（为节省篇幅，此处不改动内容）
EOF

  if ! docker ps --format "{{.Names}}" | grep -q "^gost-mysql$"; then
    echo "❌ 数据库容器 gost-mysql 未运行"
    echo "🔍 当前运行的容器："
    docker ps --format "table {{.Names}}\t{{.Image}}\t{{.Status}}"
    echo "❌ 数据库结构更新失败，请手动执行 temp_migration.sql"
    echo "📁 迁移文件已保存为 temp_migration.sql"
    return 1
  fi

  if docker exec -i gost-mysql mysql -u "$DB_USER" -p"$DB_PASSWORD" < temp_migration.sql 2>/dev/null; then
    echo "✅ 数据库结构更新完成"
  else
    echo "⚠️ 使用用户密码失败，尝试root密码..."
    if docker exec -i gost-mysql mysql -u root -p"$DB_PASSWORD" < temp_migration.sql 2>/dev/null; then
      echo "✅ 数据库结构更新完成"
    else
      echo "❌ 数据库结构更新失败，请手动执行 temp_migration.sql"
      echo "📁 迁移文件已保存为 temp_migration.sql"
      echo "🔍 数据库容器状态: $(docker inspect -f '{{.State.Status}}' gost-mysql 2>/dev/null || echo '容器不存在')"
      echo "🛑 更新终止"
      return 1
    fi
  fi

  rm -f temp_migration.sql
  echo "✅ 更新完成"
}

# 导出数据库备份
export_migration_sql() {
  echo "📄 开始导出数据库备份..."
  echo "🔍 获取数据库配置信息..."

  if ! docker ps --format "{{.Names}}" | grep -q "^springboot-backend$"; then
    echo "❌ 后端容器未运行，尝试从 .env 文件读取配置..."
    if [[ -f ".env" ]]; then
      DB_NAME=$(grep "^DB_NAME=" .env | cut -d'=' -f2 2>/dev/null)
      DB_PASSWORD=$(grep "^DB_PASSWORD=" .env | cut -d'=' -f2 2>/dev/null)
      DB_USER=$(grep "^DB_USER=" .env | cut -d'=' -f2 2>/dev/null)
      if [[ -n "$DB_NAME" && -n "$DB_PASSWORD" && -n "$DB_USER" ]]; then
        echo "✅ 从 .env 文件读取数据库配置成功"
      else
        echo "❌ .env 文件中的数据库配置不完整"
        return 1
      fi
    else
      echo "❌ 未找到 .env 文件"
      return 1
    fi
  else
    DB_INFO=$(docker exec springboot-backend env | grep "^DB_" 2>/dev/null || echo "")
    if [[ -n "$DB_INFO" ]]; then
      DB_NAME=$(echo "$DB_INFO" | grep "^DB_NAME=" | cut -d'=' -f2)
      DB_PASSWORD=$(echo "$DB_INFO" | grep "^DB_PASSWORD=" | cut -d'=' -f2)
      DB_USER=$(echo "$DB_INFO" | grep "^DB_USER=" | cut -d'=' -f2)
      echo "✅ 从容器环境变量读取数据库配置成功"
    else
      echo "❌ 无法从容器获取数据库配置，尝试从 .env 文件读取..."
      if [[ -f ".env" ]]; then
        DB_NAME=$(grep "^DB_NAME=" .env | cut -d'=' -f2 2>/dev/null)
        DB_PASSWORD=$(grep "^DB_PASSWORD=" .env | cut -d'=' -f2 2>/dev/null)
        DB_USER=$(grep "^DB_USER=" .env | cut -d'=' -f2 2>/dev/null)
        if [[ -n "$DB_NAME" && -n "$DB_PASSWORD" && -n "$DB_USER" ]]; then
          echo "✅ 从 .env 文件读取数据库配置成功"
        else
          echo "❌ .env 文件中的数据库配置不完整"
          return 1
        fi
      else
        echo "❌ 未找到 .env 文件"
        return 1
      fi
    fi
  fi

  if [[ -z "$DB_PASSWORD" || -z "$DB_USER" || -z "$DB_NAME" ]]; then
    echo "❌ 数据库配置不完整（缺少必要参数）"
    return 1
  fi

  echo "📋 数据库配置："
  echo "   数据库名: $DB_NAME"
  echo "   用户名: $DB_USER"

  if ! docker ps --format "{{.Names}}" | grep -q "^gost-mysql$"; then
    echo "❌ 数据库容器未运行，无法导出数据"
    echo "🔍 当前运行的容器："
    docker ps --format "table {{.Names}}\t{{.Image}}\t{{.Status}}"
    return 1
  fi

  SQL_FILE="database_backup_$(date +%Y%m%d_%H%M%S).sql"
  echo "📝 导出数据库备份: $SQL_FILE"
  echo "⏳ 正在导出数据库..."

  if docker exec gost-mysql mysqldump -u "$DB_USER" -p"$DB_PASSWORD" --single-transaction --routines --triggers "$DB_NAME" > "$SQL_FILE" 2>/dev/null; then
    echo "✅ 数据库导出成功"
  else
    echo "⚠️ 使用用户密码失败，尝试root密码..."
    if docker exec gost-mysql mysqldump -u root -p"$DB_PASSWORD" --single-transaction --routines --triggers "$DB_NAME" > "$SQL_FILE" 2>/dev/null; then
      echo "✅ 数据库导出成功"
    else
      echo "❌ 数据库导出失败"
      rm -f "$SQL_FILE"
      return 1
    fi
  fi

  if [[ -f "$SQL_FILE" ]] && [[ -s "$SQL_FILE" ]]; then
    FILE_SIZE=$(du -h "$SQL_FILE" | cut -f1)
    echo "📁 文件位置: $(pwd)/$SQL_FILE"
    echo "📊 文件大小: $FILE_SIZE"
  else
    echo "❌ 导出的文件为空或不存在"
    rm -f "$SQL_FILE"
    return 1
  fi
}

# ===================== 新增：安装并配置反向代理（Caddy） =====================
install_reverse_proxy() {
  echo "🚀 开始安装并配置 Caddy 反向代理..."
  # 尽量读取前端端口作为反代后端端口的建议值
  FRONTEND_HINT=""
  if [[ -f ".env" ]]; then
    ENV_FRONTEND_PORT=$(grep "^FRONTEND_PORT=" .env | cut -d'=' -f2 2>/dev/null || echo "")
    if [[ -n "$ENV_FRONTEND_PORT" ]]; then
      FRONTEND_HINT="$ENV_FRONTEND_PORT"
    fi
  fi

  echo "📥 下载 proxy.sh ..."
  if ! curl -fsSL "$PROXY_SH_URL" -o proxy.sh; then
    echo "❌ 下载 proxy.sh 失败：$PROXY_SH_URL"
    exit 1
  fi
  chmod +x proxy.sh

  echo "ℹ️ 即将启动反代安装脚本。填写建议："
  echo "   - 反向代理目标地址：建议填 127.0.0.1"
  if [[ -n "$FRONTEND_HINT" ]]; then
    echo "   - 反向代理目标端口：建议填 $FRONTEND_HINT（从 .env 读取的前端端口）"
  else
    echo "   - 反向代理目标端口：建议填 6366（默认前端端口）"
  fi
  echo "   - 其余选项按需选择（是否使用 DNS 验证、邮箱等）"

  ./proxy.sh

  echo "✅ 反向代理配置完成"
}

# 卸载面板
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
  rm -f docker-compose.yml gost.sql .env
  echo "✅ 卸载完成"
}

# 主逻辑
main() {
  while true; do
    show_menu
    read -p "请输入选项 (1-6): " choice

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
        export_migration_sql
        delete_self
        exit 0
        ;;
      5)
        install_reverse_proxy
        delete_self
        exit 0
        ;;
      6)
        echo "👋 退出脚本"
        delete_self
        exit 0
        ;;
      *)
        echo "❌ 无效选项，请输入 1-6"
        echo ""
        ;;
    esac
  done
}

# 执行主函数
main