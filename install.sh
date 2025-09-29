#!/bin/bash

# ClashFun 一键安装脚本
# 支持 Linux 和 macOS

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 项目信息
REPO="ink1ing/clashfun"
BINARY_NAME="cf"
INSTALL_DIR="/usr/local/bin"

# 检测操作系统和架构
detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)

    case $os in
        linux*)
            OS="linux"
            ;;
        darwin*)
            OS="darwin"
            ;;
        *)
            echo -e "${RED}❌ 不支持的操作系统: $os${NC}"
            exit 1
            ;;
    esac

    case $arch in
        x86_64|amd64)
            ARCH="x86_64"
            ;;
        aarch64|arm64)
            ARCH="aarch64"
            ;;
        *)
            echo -e "${RED}❌ 不支持的架构: $arch${NC}"
            exit 1
            ;;
    esac

    PLATFORM="${OS}-${ARCH}"
    echo -e "${BLUE}🔍 检测到平台: ${PLATFORM}${NC}"
}

# 获取最新版本
get_latest_version() {
    echo -e "${BLUE}🔍 获取最新版本信息...${NC}"

    # 尝试使用 GitHub API 获取最新版本
    if command -v curl >/dev/null 2>&1; then
        VERSION=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
    elif command -v wget >/dev/null 2>&1; then
        VERSION=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
    else
        echo -e "${RED}❌ 需要 curl 或 wget 工具${NC}"
        exit 1
    fi

    if [ -z "$VERSION" ]; then
        echo -e "${YELLOW}⚠️  无法获取版本信息，使用 main 分支${NC}"
        VERSION="main"
        DOWNLOAD_URL="https://github.com/${REPO}/archive/main.tar.gz"
    else
        echo -e "${GREEN}✅ 最新版本: ${VERSION}${NC}"
        DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/cf-${PLATFORM}.tar.gz"
    fi
}

# 检查权限
check_permissions() {
    if [ ! -w "$INSTALL_DIR" ]; then
        echo -e "${YELLOW}⚠️  需要管理员权限安装到 $INSTALL_DIR${NC}"
        if command -v sudo >/dev/null 2>&1; then
            USE_SUDO="sudo"
        else
            echo -e "${RED}❌ 无法获取管理员权限${NC}"
            exit 1
        fi
    fi
}

# 下载并安装
install_binary() {
    local temp_dir=$(mktemp -d)
    echo -e "${BLUE}📦 下载 ClashFun...${NC}"

    if [ "$VERSION" = "main" ]; then
        # 如果没有 release，提示用户需要编译
        echo -e "${YELLOW}⚠️  暂无预编译版本，需要从源码编译${NC}"
        echo -e "${BLUE}💡 请按以下步骤手动安装:${NC}"
        echo "1. 安装 Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        echo "2. 克隆项目: git clone https://github.com/${REPO}.git"
        echo "3. 编译项目: cd clashfun && cargo build --release"
        echo "4. 安装二进制: ${USE_SUDO} cp target/release/cf ${INSTALL_DIR}/"
        echo "5. 设置权限: ${USE_SUDO} chmod +x ${INSTALL_DIR}/cf"
        exit 1
    else
        # 下载预编译的二进制文件
        if command -v curl >/dev/null 2>&1; then
            curl -L "$DOWNLOAD_URL" -o "$temp_dir/cf.tar.gz"
        elif command -v wget >/dev/null 2>&1; then
            wget "$DOWNLOAD_URL" -O "$temp_dir/cf.tar.gz"
        fi

        # 解压并安装
        cd "$temp_dir"
        tar -xzf cf.tar.gz

        echo -e "${BLUE}📦 安装到 $INSTALL_DIR...${NC}"
        $USE_SUDO cp cf "$INSTALL_DIR/"
        $USE_SUDO chmod +x "$INSTALL_DIR/cf"
    fi

    # 清理临时文件
    rm -rf "$temp_dir"
}

# 验证安装
verify_installation() {
    if command -v cf >/dev/null 2>&1; then
        echo -e "${GREEN}✅ ClashFun 安装成功！${NC}"
        echo -e "${BLUE}🎮 版本信息:${NC}"
        cf --version
    else
        echo -e "${RED}❌ 安装失败，请检查 PATH 环境变量${NC}"
        echo -e "${YELLOW}💡 请将 $INSTALL_DIR 添加到 PATH 中${NC}"
        exit 1
    fi
}

# 显示使用说明
show_usage() {
    echo -e "${GREEN}🎉 安装完成！${NC}"
    echo -e "${BLUE}📖 快速开始:${NC}"
    echo "1. 设置订阅链接: cf set-subscription <URL>"
    echo "2. 查看节点列表: cf nodes"
    echo "3. 自动选择节点: cf auto-select"
    echo "4. 启动加速服务: cf start"
    echo "5. 查看服务状态: cf status"
    echo ""
    echo -e "${BLUE}📚 更多命令请查看: cf --help${NC}"
    echo -e "${BLUE}🔗 项目地址: https://github.com/${REPO}${NC}"
}

# 主安装流程
main() {
    echo -e "${GREEN}🚀 ClashFun 轻量级游戏加速器安装程序${NC}"
    echo ""

    detect_platform
    get_latest_version
    check_permissions
    install_binary
    verify_installation
    show_usage
}

# 错误处理
trap 'echo -e "${RED}❌ 安装过程中出现错误${NC}"' ERR

# 执行主程序
main "$@"