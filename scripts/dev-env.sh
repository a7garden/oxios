# Oxios developer environment setup
# Run: ./scripts/dev-env.sh

# ─── Rust ─────────────────────────────────────────────────────────
export RUSTC_WRAPPER=sccache

# ─── Optional: LLD linker for faster linking ──────────────────────
# export RUSTFLAGS="-C link-arg=-fuse-ld=lld"

# ─── Optional: Enable incremental compilation for even faster dev cycles ──
# export CARGO_INCREMENTAL=1

# ─── Start sccache server (for local dev) ─────────────────────────
start-sccache() {
  if command -v sccache &> /dev/null; then
    sccache --start-server 2>/dev/null || true
    echo "sccache status:"
    sccache --show-stats
  else
    echo "sccache not installed. Run: brew install sccache"
  fi
}

# ─── Build speed benchmark ─────────────────────────────────────────
benchmark-build() {
  echo "=== Cold build (oxios-kernel) ==="
  cargo clean -p oxios-kernel
  time cargo build -p oxios-kernel

  echo ""
  echo "=== Cached build ==="
  time cargo build -p oxios-kernel

  echo ""
  echo "=== sccache stats ==="
  sccache --show-stats 2>/dev/null || echo "sccache not running"
}

# Run on shell init
start-sccache