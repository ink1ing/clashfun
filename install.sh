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

# 检测并清理旧版本
detect_and_cleanup_old_versions() {
    echo -e "${BLUE}🔍 检测已安装的版本...${NC}"

    # 常见的安装路径
    local common_paths=(
        "/usr/local/bin/cf"
        "/usr/bin/cf"
        "/opt/clashfun/cf"
        "$HOME/.local/bin/cf"
        "$HOME/bin/cf"
    )

    # 查找旧版本ClashFun的可能名称
    local old_names=(
        "clashfun"
        "clash-fun"
        "cf.old"
        "cf.backup"
    )

    local found_versions=()
    local found_old_names=()

    # 检查常见路径中的cf命令
    for path in "${common_paths[@]}"; do
        if [ -f "$path" ]; then
            found_versions+=("$path")
        fi
    done

    # 检查PATH中的cf命令
    if command -v cf >/dev/null 2>&1; then
        local cf_path=$(which cf 2>/dev/null)
        if [ -n "$cf_path" ] && [ -f "$cf_path" ]; then
            # 避免重复添加
            local already_added=false
            for existing in "${found_versions[@]}"; do
                if [ "$existing" = "$cf_path" ]; then
                    already_added=true
                    break
                fi
            done
            if [ "$already_added" = false ]; then
                found_versions+=("$cf_path")
            fi
        fi
    fi

    # 检查旧的程序名称
    for old_name in "${old_names[@]}"; do
        if command -v "$old_name" >/dev/null 2>&1; then
            local old_path=$(which "$old_name" 2>/dev/null)
            if [ -n "$old_path" ] && [ -f "$old_path" ]; then
                found_old_names+=("$old_path")
            fi
        fi
    done

    # 检查用户自定义位置
    for path in "${common_paths[@]}"; do
        local dir=$(dirname "$path")
        if [ -d "$dir" ]; then
            for old_name in "${old_names[@]}"; do
                local old_path="$dir/$old_name"
                if [ -f "$old_path" ]; then
                    found_old_names+=("$old_path")
                fi
            done
        fi
    done

    # 显示发现的版本
    if [ ${#found_versions[@]} -gt 0 ] || [ ${#found_old_names[@]} -gt 0 ]; then
        echo -e "${YELLOW}⚠️  发现已安装的版本:${NC}"

        for version in "${found_versions[@]}"; do
            echo -e "   📁 $version"
            # 尝试获取版本信息
            if [ -x "$version" ]; then
                local version_info=$("$version" --version 2>/dev/null || echo "版本信息无法获取")
                echo -e "      版本: $version_info"
            fi
        done

        for old_version in "${found_old_names[@]}"; do
            echo -e "   📁 $old_version (旧程序名)"
        done

        echo ""
        echo -e "${BLUE}🧹 正在清理旧版本以避免冲突...${NC}"

        # 清理找到的版本
        for version in "${found_versions[@]}" "${found_old_names[@]}"; do
            if [ -f "$version" ]; then
                local file_dir=$(dirname "$version")
                if [ -w "$file_dir" ]; then
                    echo -e "   🗑️  删除: $version"
                    rm -f "$version"
                elif [ -n "$USE_SUDO" ] || command -v sudo >/dev/null 2>&1; then
                    echo -e "   🗑️  删除 (需要权限): $version"
                    sudo rm -f "$version"
                else
                    echo -e "   ❌ 无法删除: $version (权限不足)"
                fi
            fi
        done

        # 清理配置文件和缓存
        cleanup_old_configs

        echo -e "${GREEN}✅ 旧版本清理完成${NC}"
    else
        echo -e "${GREEN}✅ 未检测到旧版本${NC}"
    fi
}

# 清理旧的配置文件和缓存
cleanup_old_configs() {
    echo -e "${BLUE}🧹 清理旧的配置文件...${NC}"

    # 清理可能的配置目录
    local config_dirs=(
        "$HOME/.config/cf"
        "$HOME/.config/clashfun"
        "$HOME/.clashfun"
        "$HOME/.cf"
    )

    # 清理可能的缓存目录
    local cache_dirs=(
        "$HOME/.cache/cf"
        "$HOME/.cache/clashfun"
        "/tmp/clashfun"
        "/tmp/cf"
    )

    for config_dir in "${config_dirs[@]}"; do
        if [ -d "$config_dir" ]; then
            echo -e "   🗑️  清理配置: $config_dir"
            rm -rf "$config_dir"
        fi
    done

    for cache_dir in "${cache_dirs[@]}"; do
        if [ -d "$cache_dir" ]; then
            echo -e "   🗑️  清理缓存: $cache_dir"
            rm -rf "$cache_dir"
        fi
    done
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

        # 获取安装路径
        local installed_path=$(which cf)
        echo -e "${BLUE}📍 安装位置: ${installed_path}${NC}"

        # 显示版本信息
        echo -e "${BLUE}🎮 版本信息:${NC}"
        cf --version

        # 检查是否还有其他cf命令存在
        echo -e "${BLUE}🔍 验证版本唯一性...${NC}"
        local all_cf_paths=$(which -a cf 2>/dev/null | head -5)
        local cf_count=$(echo "$all_cf_paths" | wc -l)

        if [ "$cf_count" -gt 1 ]; then
            echo -e "${YELLOW}⚠️  系统中发现多个cf命令:${NC}"
            echo "$all_cf_paths" | while read -r path; do
                if [ -n "$path" ]; then
                    echo -e "   📁 $path"
                fi
            done
            echo -e "${YELLOW}💡 建议运行 'cf update' 来统一版本${NC}"
        else
            echo -e "${GREEN}✅ 系统中只有一个cf命令，版本统一${NC}"
        fi
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
    echo -e "${BLUE}🔄 版本管理:${NC}"
    echo "• 检查更新: cf update"
    echo "• 交互模式: cf (然后使用 /update)"
    echo "• 重新安装: curl -fsSL https://raw.githubusercontent.com/${REPO}/master/install.sh | sh"
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
    detect_and_cleanup_old_versions
    install_binary
    verify_installation
    show_usage
}

# 错误处理
trap 'echo -e "${RED}❌ 安装过程中出现错误${NC}"' ERR

# 执行主程序
main "$@"