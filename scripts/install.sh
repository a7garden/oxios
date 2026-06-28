#!/bin/bash
set -euo pipefail

# ═══════════════════════════════════════════════════════════
# Oxios Agent OS — Install Script
# ═══════════════════════════════════════════════════════════

VERSION="${VERSION:-latest}"
readonly INSTALL_DIR="${HOME}/.oxios/bin"
readonly REPO="a7garden/oxios"

info()  { echo "[oxios] $*" >&2; }
warn()  { echo "[oxios] WARNING: $*" >&2; }
error() { echo "[oxios] ERROR: $*" >&2; exit 1; }

# ── Platform guard ─────────────────────────────────────────────
# Prebuilt binaries target macOS Apple Silicon (aarch64-apple-darwin)
# only. Any other OS/arch is rejected with a pointer to `cargo install`
# so the script never silently installs a binary that won't run.
detect_platform() {
    local os="$(uname -s)"
    local arch="$(uname -m)"

    if [ "$os" != "Darwin" ] || [ "$arch" != "arm64" ]; then
        cat >&2 <<EOF
[oxios] ERROR: Prebuilt binaries are macOS Apple Silicon (aarch64-apple-darwin) only.
[oxios] ERROR: Detected: ${os}/${arch}. Install from source instead:
[oxios] ERROR:   cargo install oxios
[oxios] ERROR: See https://github.com/${REPO} for details.
EOF
        exit 1
    fi
    echo "macos-arm64"
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

    # Prebuilt asset is a tarball named by its Rust target triple.
    local asset="oxios-aarch64-apple-darwin.tar.gz"
    local dest="${INSTALL_DIR}/${binary_name}"

    # Stage the download in a temp dir so a failed/aborted install
    # never leaves a half-written binary in INSTALL_DIR. `tmpdir` is
    # deliberately NOT local: the EXIT trap fires after install()
    # returns, when a local would be out of scope under `set -u`.
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    local archive="${tmpdir}/${asset}"
    curl -fsSL "${base_url}/${asset}" -o "$archive" \
        || error "Download failed"

    # Verify checksum. `shasum` ships with macOS; `sha256sum` does not.
    if curl -fsSL "${base_url}/${asset}.sha256" -o "${tmpdir}/${asset}.sha256"; then
        info "Verifying checksum..."
        ( cd "$tmpdir" && shasum -a 256 -c "${asset}.sha256" >/dev/null 2>&1 ) \
            || error "Checksum mismatch — the download may be corrupt or tampered."
        info "Checksum verified."
    else
        warn "No checksum sidecar in release ${tag#v}; skipping verification."
    fi

    # Extract and move into place (overwrites a previous install).
    tar -xzf "$archive" -C "$tmpdir"
    [ -f "${tmpdir}/oxios" ] || error "Archive did not contain an 'oxios' binary."
    mv -f "${tmpdir}/oxios" "$dest"
    chmod +x "$dest"

    # Add to PATH.
    local shell="${SHELL##*/}"
    local shell_rc=""
    case "$shell" in
        zsh)  shell_rc="${HOME}/.zshrc" ;;
        bash) shell_rc="${HOME}/.bashrc" ;;
        fish) shell_rc="${HOME}/.config/fish/config.fish" ;;
        *)    shell_rc="${HOME}/.profile" ;;
    esac

    # Create the rc file if missing (fish's config dir may not exist yet).
    if [ "$shell" = "fish" ]; then
        mkdir -p "$(dirname "$shell_rc")"
    fi
    touch "$shell_rc" 2>/dev/null || true

    # Idempotent: skip if any oxios/bin PATH line is already present.
    # `export PATH=...` is invalid in fish — use fish_add_path there.
    if ! grep -qF '.oxios/bin' "$shell_rc"; then
        {
            printf '\n# Oxios Agent OS\n'
            if [ "$shell" = "fish" ]; then
                printf 'fish_add_path ~/.oxios/bin\n'
            else
                printf 'export PATH="$HOME/.oxios/bin:$PATH"\n'
            fi
        } >> "$shell_rc"
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