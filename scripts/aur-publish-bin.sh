#!/bin/bash
# AUR 自动发布脚本 for dnstest-bin (二进制版本)
# 直接使用预编译二进制文件，无需构建

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${GREEN}[INFO]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

# 配置
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
AUR_DIR="$PROJECT_DIR/.aur-build-bin"
AUR_REMOTE="ssh://aur@aur.archlinux.org/dnstest-bin.git"

# 清理函数
cleanup() {
    if [ -d "$AUR_DIR" ]; then
        info "清理临时文件 ..."
        rm -rf "$AUR_DIR"
    fi
}

trap cleanup EXIT

# 从 Cargo.toml 读取版本
get_version() {
    grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/'
}

# 计算 sha256sum
calc_sha256() {
    sha256sum "$1" | cut -d' ' -f1
}

# 生成 PKGBUILD
generate_pkgbuild() {
    local version="$1"
    local linux_sha256="$2"

    cat > "$AUR_DIR/PKGBUILD" << EOF
# Maintainer: wjsoj <wjs@wjsphy.top>

pkgname=dnstest-bin
pkgver=${version}
pkgrel=1
pkgdesc="A modern DNS speed testing and pollution detection CLI tool (pre-built binary)"
arch=('x86_64')
url="https://github.com/wjsoj/dnstest"
license=('MIT')
provides=('dnstest')
conflicts=('dnstest')
options=('!debug')
source_x86_64=("dnstest-\$pkgver.tar.gz::https://github.com/wjsoj/dnstest/releases/download/v\$pkgver/dnstest-x86_64-unknown-linux-gnu.tar.gz")
sha256sums_x86_64=('${linux_sha256}')

package() {
  install -Dm755 "\$srcdir/dnstest" "\$pkgdir/usr/bin/dnstest"
}
EOF
}

# 生成 .SRCINFO
generate_srcinfo() {
    info "生成 .SRCINFO ..."
    cd "$AUR_DIR"
    makepkg --printsrcinfo > .SRCINFO
}

# 克隆 AUR 仓库
setup_aur_repo() {
    info "准备 AUR 仓库 ..."
    rm -rf "$AUR_DIR"

    info "克隆 AUR 仓库 ..."
    info "URL: $AUR_REMOTE"
    git clone "$AUR_REMOTE" "$AUR_DIR" || {
        error "无法克隆 AUR 仓库，请确认 SSH 密钥已配置"
    }
    info "AUR 仓库克隆完成"
}

# 提交并推送
commit_and_push() {
    local version="$1"

    cd "$AUR_DIR"

    git add PKGBUILD .SRCINFO

    if git diff --cached --quiet; then
        warn "没有变更需要提交"
        return 0
    fi

    git commit -m "Update to v${version}"

    # 确保 branch 名为 master
    local current_branch=$(git branch --show-current)
    if [ "$current_branch" != "master" ]; then
        info "重命名分支 $current_branch 为 master..."
        git branch -m master
    fi

    info "推送到 AUR (可能需要输入 SSH 密钥)..."
    git push origin master || error "推送失败，请检查 SSH 配置和网络连接"
    info "推送成功"
}

# 主流程
main() {
    cd "$PROJECT_DIR"

    VERSION=$(get_version)
    info "当前版本: $VERSION"

    # 获取 GitHub release 压缩包的 sha256
    info "获取 Linux x86_64 压缩包的 sha256 ..."
    local archive_url="https://github.com/wjsoj/dnstest/releases/download/v${VERSION}/dnstest-x86_64-unknown-linux-gnu.tar.gz"
    local tmpfile=$(mktemp --suffix=.tar.gz)
    info "下载: $archive_url"
    curl -L "$archive_url" -o "$tmpfile" || error "下载失败，请检查网络和 release 是否存在"
    SHA256=$(calc_sha256 "$tmpfile")
    rm -f "$tmpfile"
    info "SHA256: $SHA256"

    # 设置 AUR 仓库
    setup_aur_repo

    # 生成 PKGBUILD
    info "生成 PKGBUILD ..."
    generate_pkgbuild "$VERSION" "$SHA256"

    # 生成 .SRCINFO
    generate_srcinfo

    # 提交并推送
    commit_and_push "$VERSION"

    echo ""
    echo -e "${GREEN}======================================${NC}"
    echo -e "${GREEN}✓ 完成! dnstest-bin v${VERSION} 已发布到 AUR${NC}"
    echo -e "${GREEN}======================================${NC}"
    echo ""
    info "安装命令: yay -S dnstest-bin"
}

main "$@"
