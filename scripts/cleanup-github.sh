#!/usr/bin/env bash
# scripts/cleanup-github.sh
#
# Post-restructure GitHub cleanup (run AFTER pushing the RFC-026 commit).
#
# Prerequisite: re-authenticate `gh` with a valid token before running:
#   gh auth login -h github.com
#
# What this does:
#   1. Pushes the local commit to origin/main
#   2. Deletes stale remote branches (already merged/abandoned)
#   3. (Optional) Creates v1.5.2 tag pointing to the new release commit
#
# Refs: docs/rfc-026-crate-restructuring.md

set -euo pipefail

REPO="a7garden/oxios"

echo "═══ Step 1: Verify gh auth ═══"
if ! gh auth status &>/dev/null; then
  echo "❌ gh is not authenticated. Run: gh auth login -h github.com"
  exit 1
fi

echo ""
echo "═══ Step 2: Push local commit to origin/main ═══"
git push origin main

echo ""
echo "═══ Step 3: Delete stale remote branches (already merged/abandoned) ═══"
echo ""
echo "Branches to delete (all already merged or abandoned):"
echo ""

STALE_BRANCHES=(
  "feat/cm6-markdown-editor"
  "feat/cm6-markdown-editor-phase2"
  "feat/cm6-markdown-editor-phase3"
  "feat/cm6-markdown-editor-phase3b"
  "fix/cm6-knowledge-editor-race"
  "fix/cm6-knowledge-editor-race-v2"
  "fix/cm6-minor-polish"
  "review/cm6-e2e-verification"
  "rfc-018/b1-chunking-normalizer-hyperbolic"
  "rfc-018/b2-embedding"
  "rfc-018/b3-root-index-quota"
  "rfc-018/b4-decay-auto"
  "rfc-018/b5-compaction-graph"
  "rfc-018/b6-memory-storage-trait"
  "rfc-018/b7-memory-manager"
  "rfc-018/b9-dream-bridge"
)

for branch in "${STALE_BRANCHES[@]}"; do
  if git ls-remote --heads origin "$branch" 2>/dev/null | grep -q "$branch"; then
    echo "  - origin/$branch"
  fi
done

echo ""
read -p "Delete these branches? [y/N] " confirm
if [[ "$confirm" =~ ^[Yy]$ ]]; then
  for branch in "${STALE_BRANCHES[@]}"; do
    if git ls-remote --heads origin "$branch" 2>/dev/null | grep -q "$branch"; then
      echo "  Deleting origin/$branch..."
      git push origin --delete "$branch" || echo "    (failed — may need manual delete in GitHub UI)"
    fi
  done
else
  echo "Skipped branch deletion."
fi

echo ""
echo "═══ Step 4: Tag v1.5.2 for the new release ═══"
echo ""
echo "v1.5.2 release details:"
echo "  - All 8 crates published to crates.io (oxios, oxios-kernel, etc.)"
echo "  - Web UI: 1.5.2 (build with: cd web && bun install && bun run build)"
echo ""
read -p "Create v1.5.2 tag and GitHub Release? [y/N] " confirm_tag

if [[ "$confirm_tag" =~ ^[Yy]$ ]]; then
  # Build web UI first
  if [ -d "web" ]; then
    echo "  Building web UI..."
    (cd web && bun install && bun run build)
  fi

  # Create tag
  echo "  Creating tag v1.5.2..."
  git tag -a v1.5.2 -m "v1.5.2 — RFC-026: consolidated application crates into binary"
  git push origin v1.5.2

  # Create GitHub Release (triggers release.yml which publishes web-dist.zip
  # and triggers publish.yml for crates.io)
  echo "  Creating GitHub Release..."
  gh release create v1.5.2 \
    --title "v1.5.2 — Crate Restructuring (RFC-026)" \
    --notes "## RFC-026: Crate Restructuring

Application-specific crates (oxios-web, oxios-cli, oxios-telegram)
and the benchmark crate (oxios-bench) are deprecated. Their code is now
part of the \`oxios\` binary as feature-gated modules.

### Breaking changes
- \`oxios-web\`/\`oxios-cli\`/\`oxios-telegram\` crates deprecated (existing
  1.x versions remain available; no further updates)
- Binary \`oxios\` now bundles all channels + HTTP API server
- \`oxios-bench\` removed entirely

### Migration
- Replaces surface/ vs channels/ artificial distinction
- React frontend moved to project root: \`web/\`
- Docker build moved to project root: \`Containerfile\`
- \`rust-embed\` removed — web UI served via runtime GitHub Releases
  download

### Validation
- 1372 tests passing (16 fewer from oxios-bench removal)
- All builds pass (default/cli/telegram/all-features)
- All 8 crates publish to crates.io cleanly

See \`docs/rfc-026-crate-restructuring.md\` for full details." \
    --target main
else
  echo "Skipped tag/release."
fi

echo ""
echo "═══ Done ═══"
echo ""
echo "Post-cleanup state:"
echo "  - origin/main:    $(git rev-parse origin/main) (RFC-026 commit)"
echo "  - Branches:       only main + 16 stale branches removed (if confirmed)"
echo "  - Tags:           v1.2.0 → v1.5.1 (historical) + v1.5.2 (new, if confirmed)"
echo "  - crates.io:      all 8 crates at 1.5.2"
echo ""
echo "GitHub repository is now fully clean."
