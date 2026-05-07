#!/usr/bin/env bash
# ─── Oxios Load Test ─────────────────────────────────────────────────────────
#
# Smoke-test the Oxios gateway under concurrent load.
#
# Usage:
#   ./scripts/load-test.sh [--host HOST] [--port PORT] [--concurrency N] [--requests N]
#
# Defaults:
#   host         127.0.0.1
#   port         4200
#   concurrency  5
#   requests     20
#
# Prerequisites:
#   - Oxios server running (cargo run)
#   - curl with --parallel support (curl 7.66+)
#   - jq (optional, for JSON pretty-printing)
#
# ───────────────────────────────────────────────────────────────────────────────

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────
HOST="127.0.0.1"
PORT=4200
CONCURRENCY=5
REQUESTS=20

# ── Argument parsing ─────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --host)         HOST="$2"; shift 2 ;;
        --port)         PORT="$2"; shift 2 ;;
        --concurrency)  CONCURRENCY="$2"; shift 2 ;;
        --requests)     REQUESTS="$2"; shift 2 ;;
        -h|--help)
            head -20 "$0" | grep '^#' | sed 's/^# \?//'
            exit 0
            ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

BASE_URL="http://${HOST}:${PORT}"

# ── Colors ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

info()  { echo -e "${CYAN}[INFO]${NC}  $*"; }
ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
fail()  { echo -e "${RED}[FAIL]${NC}  $*"; }

# ── Pre-flight checks ────────────────────────────────────────────────────────
info "Oxios Load Test"
info "  Target:     ${BASE_URL}"
info "  Requests:   ${REQUESTS}"
info "  Concurrency: ${CONCURRENCY}"
echo ""

if ! command -v curl &>/dev/null; then
    fail "curl is required but not found in PATH"
    exit 1
fi

# Check server is reachable
info "Checking server availability..."
if ! curl -sf "${BASE_URL}/api/health" -o /dev/null 2>/dev/null; then
    warn "Health endpoint not available (server may not be running or /api/health not defined)"
    warn "Continuing anyway — tests will fail if server is down..."
fi
echo ""

# ── Helper: measure a single request ─────────────────────────────────────────
# Outputs: HTTP_CODE TIME_TOTAL
measure() {
    local endpoint="$1"
    local method="${2:-GET}"
    local data="${3:-}"

    local args=(-s -o /dev/null -w "%{http_code} %{time_total}")
    if [[ "$method" == "POST" && -n "$data" ]]; then
        args+=(-X POST -H "Content-Type: application/json" -d "$data")
    fi

    curl "${args[@]}" "${BASE_URL}${endpoint}" 2>/dev/null || echo "000 99.999"
}

# ── Test 1: Health endpoint ──────────────────────────────────────────────────
info "Test 1: Health check"
read -r code time <<< "$(measure "/api/health")"
if [[ "$code" =~ ^(200|404)$ ]]; then
    ok "Health endpoint responded (HTTP ${code}, ${time}s)"
else
    warn "Health endpoint returned HTTP ${code} (${time}s) — may be expected if endpoint not defined"
fi

# ── Test 2: Status endpoint ──────────────────────────────────────────────────
info "Test 2: Status check"
read -r code time <<< "$(measure "/api/status")"
if [[ "$code" =~ ^(200|404)$ ]]; then
    ok "Status endpoint responded (HTTP ${code}, ${time}s)"
else
    warn "Status endpoint returned HTTP ${code} (${time}s)"
fi

# ── Test 3: Concurrent message sends ─────────────────────────────────────────
info "Test 3: Concurrent message load (${REQUESTS} requests, ${CONCURRENCY} parallel)"

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

PAYLOAD='{"message":"Hello from load test","channel":"load-test"}'

SUCCESS=0
FAIL=0
TOTAL_TIME=0.0

for i in $(seq 1 "$REQUESTS"); do
    (
        read -r code time <<< "$(measure "/api/message" "POST" "$PAYLOAD")"
        echo "${code} ${time}" > "${TMPDIR}/result_${i}"
    ) &

    # Throttle to CONCURRENCY parallel jobs
    if (( i % CONCURRENCY == 0 )); then
        wait
    fi
done
wait  # Wait for all remaining background jobs

# Collect results
for f in "${TMPDIR}"/result_*; do
    read -r code time < "$f"
    TOTAL_TIME=$(echo "$TOTAL_TIME + $time" | bc -l)
    if [[ "$code" =~ ^(200|201|202|204)$ ]]; then
        ((SUCCESS++)) || true
    else
        ((FAIL++)) || true
    fi
done

AVG_TIME=$(echo "scale=4; $TOTAL_TIME / $REQUESTS" | bc -l)

echo ""
info "Results:"
echo "  Total requests:  ${REQUESTS}"
echo "  Successful (2xx): ${SUCCESS}"
echo "  Failed:           ${FAIL}"
echo "  Avg response:     ${AVG_TIME}s"
echo "  Total time:       ${TOTAL_TIME}s"

if (( FAIL == 0 )); then
    ok "All requests succeeded"
else
    warn "${FAIL} request(s) failed (non-2xx response)"
fi

# ── Test 4: WebSocket upgrade ────────────────────────────────────────────────
info "Test 4: WebSocket upgrade check"
WS_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
    -H "Upgrade: websocket" \
    -H "Connection: Upgrade" \
    -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
    -H "Sec-WebSocket-Version: 13" \
    "${BASE_URL}/ws" 2>/dev/null || echo "000")

if [[ "$WS_CODE" == "101" ]]; then
    ok "WebSocket upgrade accepted (HTTP 101)"
elif [[ "$WS_CODE" == "000" ]]; then
    warn "Server not reachable for WebSocket upgrade"
else
    warn "WebSocket upgrade returned HTTP ${WS_CODE} (expected 101)"
fi

# ── Test 5: Static file serving ──────────────────────────────────────────────
info "Test 5: Static file serving"
read -r code time <<< "$(measure "/index.html")"
if [[ "$code" == "200" ]]; then
    ok "Static file served (HTTP 200, ${time}s)"
elif [[ "$code" == "000" ]]; then
    warn "Server not reachable for static files"
else
    warn "Static file returned HTTP ${code} (${time}s)"
fi

# ── Summary ──────────────────────────────────────────────────────────────────
echo ""
info "Load test complete."
