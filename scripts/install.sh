#!/bin/bash
set -euo pipefail

# ═══════════════════════════════════════════════════════════
# Oxios Agent OS — Install Script
# ═══════════════════════════════════════════════════════════

readonly VERSION="${VERSION:-latest}"
readonly INSTALL_DIR="${HOME}/.oxios/bin"
readonly REPO="a7garden/oxios"

info()  { echo "[oxios] $*" >&2; }
warn()  { echo "[oxios] WARNING: $*" >&2; }
error() { echo "[oxios] ERROR: $*" >&2; exit 1; }

# ── Detect OS & Architecture ──────────────────────────────────
detect_platform() {
    local os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    local arch="$(uname -m)"

    case "$arch" in
        arm64|aarch64) arch="arm64" ;;
        x86_64)        arch="x86_64" ;;
        *)             error "Unsupported architecture: $arch" ;;
    esac

    case "$os" in
        darwin) os="macos" ;;
        linux)  os="linux" ;;
        *)      error "Unsupported OS: $os" ;;
    esac

    echo "${os}-${arch}"
}

# ── Download & Install ────────────────────────────────────────
install() {
    local platform="$(detect_platform)"
    local binary_name="oxios"
    local download_url

    info "Detected platform: $platform"
    info "Installing Oxios $VERSION to $INSTALL_DIR"

    mkdir -p "$INSTALL_DIR"

    if [ "$VERSION" = "latest" ]; then
        local latest
        latest=$(curl -sSL "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep -o '"tag_name": "[^"]*"' | cut -d'"' -f4)
        VERSION="${latest#v}"
        info "Latest version: $VERSION"
    fi

    local tag="v${VERSION#v}"
    local base_url="https://github.com/${REPO}/releases/download/${tag}"

    info "Downloading from ${base_url}"

    # Download binary.
    local dest="${INSTALL_DIR}/${binary_name}"
    local checksum_url="${base_url}/${binary_name}.sha256"

    curl -sSL "${base_url}/${binary_name}" -o "$dest" \
        || error "Download failed"

    # Verify checksum.
    if curl -sf "$checksum_url" > /dev/null; then
        info "Verifying checksum..."
        local expected
        expected=$(curl -sSL "$checksum_url" | awk '{print $1}')
        local actual
        actual=$(sha256sum "$dest" | awk '{print $1}')
        if [ "$expected" != "$actual" ]; then
            rm -f "$dest"
            error "Checksum mismatch. Please try again."
        fi
        info "Checksum verified."
    fi

    chmod +x "$dest"

    # Add to PATH.
    local shell_rc=""
    case "${SHELL##*/}" in
        zsh) shell_rc="${HOME}/.zshrc" ;;
        bash) shell_rc="${HOME}/.bashrc" ;;
        fish) shell_rc="${HOME}/.config/fish/config.fish" ;;
        *) shell_rc="${HOME}/.profile" ;;
    esac

    if [ -f "$shell_rc" ] && ! grep -q '"\$HOME/.oxios/bin"' "$shell_rc"; then
        echo '' >> "$shell_rc"
        echo '# Oxios Agent OS' >> "$shell_rc"
        echo 'export PATH="$HOME/.oxios/bin:$PATH"' >> "$shell_rc"
        info "Added $INSTALL_DIR to PATH (restart shell or source $shell_rc)"
    fi

    info "✅ Oxios $VERSION installed successfully!"
    info "   Run: oxios"
}

# ── Main ──────────────────────────────────────────────────────
main() {
    info "Oxios Agent OS Installer"
    info "────────────────────────"

    if ! command -v curl > /dev/null 2>&1; then
        error "curl is required but not installed"
    fi

    install
}

main "$@"