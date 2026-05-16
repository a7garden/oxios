#!/usr/bin/env bash
# Oxios Benchmark Runner
# Usage: ./runner.sh [--scenario S01] [--skip-cleanup] [--verbose]
#
# 모든 시나리오를 순차 실행하고 보고서를 생성한다.
# pi-agent가 직접 실행하거나, 사람이 수동으로 실행할 수 있다.

set -euo pipefail

# ─── Colors ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# ─── Config ──────────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TIMESTAMP="$(date +%Y-%m-%d-%H%M%S)"
REPORT_DIR="${SCRIPT_DIR}/reports/benchmark-${TIMESTAMP}"
SCENARIOS_DIR="${SCRIPT_DIR}/scenarios"
VERBOSE=0
SKIP_CLEANUP=0
TARGET_SCENARIO=""
OXIOS_HOME="${OXIOS_HOME:-$HOME/.oxios}"
OXIOS_BIN="${OXIOS_BIN:-oxios}"

# ─── Argument Parsing ───────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case $1 in
        --scenario) TARGET_SCENARIO="$2"; shift 2 ;;
        --skip-cleanup) SKIP_CLEANUP=1; shift ;;
        --verbose) VERBOSE=1; shift ;;
        --help|-h)
            echo "Usage: $0 [--scenario S01] [--skip-cleanup] [--verbose]"
            echo ""
            echo "Options:"
            echo "  --scenario S01    Run only the specified scenario"
            echo "  --skip-cleanup    Don't clean up workspace after benchmark"
            echo "  --verbose         Show full command output"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# ─── Helper Functions ────────────────────────────────────────────────────────
log_header() {
    echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${CYAN}  $1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"
}

log_step() {
    echo -e "${GREEN}  ✓ $1${NC}"
}

log_warn() {
    echo -e "${YELLOW}  ⚠ $1${NC}"
}

log_error() {
    echo -e "${RED}  ✗ $1${NC}"
}

log_info() {
    echo -e "${BLUE}  ℹ $1${NC}"
}

# Run a command and capture results
# Usage: run_cmd <label> <command> [args...]
# Sets: CMD_EXIT_CODE, CMD_DURATION_MS, CMD_STDOUT, CMD_STDERR
run_cmd() {
    local label="$1"
    shift
    local cmd="$*"
    local start end

    if [[ $VERBOSE -eq 1 ]]; then
        echo -e "  ${CYAN}\$ ${cmd}${NC}"
    else
        echo -ne "  ${CYAN}\$ ${cmd}${NC} ... "
    fi

    local stdout_file stderr_file
    stdout_file=$(mktemp)
    stderr_file=$(mktemp)

    start=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
    set +e
    eval "$cmd" > "$stdout_file" 2> "$stderr_file"
    CMD_EXIT_CODE=$?
    set -e
    end=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")

    CMD_DURATION_MS=$((end - start))
    CMD_STDOUT=$(cat "$stdout_file")
    CMD_STDERR=$(cat "$stderr_file")
    rm -f "$stdout_file" "$stderr_file"

    if [[ $VERBOSE -eq 1 ]]; then
        if [[ $CMD_EXIT_CODE -eq 0 ]]; then
            log_step "$label (${CMD_DURATION_MS}ms)"
        else
            log_error "$label (exit=$CMD_EXIT_CODE, ${CMD_DURATION_MS}ms)"
        fi
        if [[ -n "$CMD_STDOUT" ]]; then
            echo "    stdout: $(echo "$CMD_STDOUT" | head -5)"
        fi
        if [[ -n "$CMD_STDERR" ]]; then
            echo "    stderr: $(echo "$CMD_STDERR" | head -3)"
        fi
    else
        if [[ $CMD_EXIT_CODE -eq 0 ]]; then
            echo -e "${GREEN}✓${NC} (${CMD_DURATION_MS}ms)"
        else
            echo -e "${RED}✗ exit=$CMD_EXIT_CODE${NC} (${CMD_DURATION_MS}ms)"
        fi
    fi
}

# Write a scenario result JSON
write_result() {
    local scenario_id="$1"
    local scenario_name="$2"
    local score="$3"
    local max_score="$4"
    local notes="$5"
    local result_file="${REPORT_DIR}/${scenario_id}.result.json"

    cat > "$result_file" << RESULTEOF
{
  "scenario_id": "${scenario_id}",
  "scenario_name": "${scenario_name}",
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "duration_ms": ${SCENARIO_DURATION_MS:-0},
  "score": ${score},
  "max_score": ${max_score},
  "notes": "${notes}",
  "steps": ${SCENARIO_STEPS:-[]}
}
RESULTEOF
    log_info "Result written to ${result_file}"
}

# ─── Pre-flight Checks ───────────────────────────────────────────────────────
preflight() {
    log_header "Pre-flight Checks"

    # Check oxios binary
    if ! command -v "$OXIOS_BIN" &>/dev/null; then
        log_error "oxios not found in PATH. Set OXIOS_BIN or install oxios."
        exit 1
    fi
    log_step "oxios found: $(which "$OXIOS_BIN")"

    # Check jq
    if ! command -v jq &>/dev/null; then
        log_warn "jq not found. JSON parsing will be limited."
    else
        log_step "jq found: $(which jq)"
    fi

    # Check API keys
    if [[ -z "${ANTHROPIC_API_KEY:-}" ]] && [[ -z "${OPENAI_API_KEY:-}" ]]; then
        log_warn "No API keys detected. Scenarios requiring LLM may fail."
    else
        log_step "API keys detected"
    fi

    # Create report directory
    mkdir -p "$REPORT_DIR"
    log_step "Report directory: $REPORT_DIR"
}

# ─── Cleanup ─────────────────────────────────────────────────────────────────
cleanup() {
    if [[ $SKIP_CLEANUP -eq 1 ]]; then
        log_info "Skipping cleanup (--skip-cleanup)"
        return
    fi
    log_header "Cleanup"
    # Stop daemon if running
    $OXIOS_BIN stop 2>/dev/null || true
    log_step "Daemon stopped"
}

# ─── Generate Report ─────────────────────────────────────────────────────────
generate_report() {
    log_header "Generating Report"

    local report_file="${REPORT_DIR}/summary.md"
    local total_score=0
    local total_max=0
    local total_duration=0
    local scenario_count=0
    local pass_count=0

    # Collect results
    local results=()
    for result_file in "${REPORT_DIR}"/*.result.json; do
        [[ -f "$result_file" ]] || continue
        results+=("$result_file")

        local score max duration
        score=$(jq -r '.score // 0' "$result_file")
        max=$(jq -r '.max_score // 10' "$result_file")
        duration=$(jq -r '.duration_ms // 0' "$result_file")
        total_score=$((total_score + score))
        total_max=$((total_max + max))
        total_duration=$((total_duration + duration))
        scenario_count=$((scenario_count + 1))
        if [[ $score -ge $((max * 7 / 10)) ]]; then
            pass_count=$((pass_count + 1))
        fi
    done

    local percentage=0
    if [[ $total_max -gt 0 ]]; then
        percentage=$((total_score * 100 / total_max))
    fi

    local grade="D"
    if [[ $percentage -ge 90 ]]; then grade="S";
    elif [[ $percentage -ge 80 ]]; then grade="A";
    elif [[ $percentage -ge 70 ]]; then grade="B";
    elif [[ $percentage -ge 60 ]]; then grade="C"; fi

    local total_min=$((total_duration / 60000))
    local total_sec=$(( (total_duration % 60000) / 1000 ))

    # Write report
    cat > "$report_file" << REPORTEOF
# Oxios Benchmark Report

**일시:** $(date '+%Y-%m-%d %H:%M:%S')
**oxios 버전:** $($OXIOS_BIN --version 2>/dev/null || echo "unknown")
**런타임:** $(uname -s) / $(rustc --version 2>/dev/null || echo "unknown")
**총 소요 시간:** ${total_min}분 ${total_sec}초

---

## 종합 결과

| 지표 | 값 |
|------|------|
| **총점** | **${total_score}/${total_max} (${percentage}%)** |
| **등급** | **${grade}** |
| **통과율** | ${pass_count}/${scenario_count} 시나리오 |
| **총 소요시간** | ${total_min}분 ${total_sec}초 |

### 등급 기준
| 등급 | 점수 범위 |
|------|-----------|
| S | 90-100% |
| A | 80-89% |
| B | 70-79% |
| C | 60-69% |
| D | <60% |

---

## 시나리오별 결과

| ID | 시나리오 | 점수 | 소요시간 | 비고 |
|----|----------|------|----------|------|
REPORTEOF

    for result_file in "${results[@]}"; do
        local id name score max duration notes
        id=$(jq -r '.scenario_id' "$result_file")
        name=$(jq -r '.scenario_name' "$result_file")
        score=$(jq -r '.score' "$result_file")
        max=$(jq -r '.max_score' "$result_file")
        duration=$(jq -r '.duration_ms' "$result_file")
        notes=$(jq -r '.notes' "$result_file" | head -1)
        local dur_sec=$((duration / 1000))
        printf "| %s | %s | %s/%s | %ss | %s |\n" "$id" "$name" "$score" "$max" "$dur_sec" "${notes:---}" >> "$report_file"
    done

    cat >> "$report_file" << 'REPORTEOF'

---

## 상세 이슈

*각 시나리오의 상세 로그는 동일 디렉토리의 `<scenario-id>.log` 파일을 참조.*

---

## 추천 사항

*벤치마크 완료 후 pi-agent가 분석하여 작성.*

REPORTEOF

    log_step "Report written to ${report_file}"

    # Print summary
    echo ""
    echo -e "${CYAN}╔══════════════════════════════════════════════════════╗${NC}"
    echo -e "${CYAN}║                  BENCHMARK RESULT                    ║${NC}"
    echo -e "${CYAN}╠══════════════════════════════════════════════════════╣${NC}"
    printf "${CYAN}║${NC}  Score: %-40s ${CYAN}║${NC}\n" "${total_score}/${total_max} (${percentage}%)"
    printf "${CYAN}║${NC}  Grade: %-40s ${CYAN}║${NC}\n" "${grade}"
    printf "${CYAN}║${NC}  Pass:  %-40s ${CYAN}║${NC}\n" "${pass_count}/${scenario_count} scenarios"
    printf "${CYAN}║${NC}  Time:  %-40s ${CYAN}║${NC}\n" "${total_min}m ${total_sec}s"
    echo -e "${CYAN}╚══════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "Full report: ${BLUE}${report_file}${NC}"
}

# ─── Scenario Runner ─────────────────────────────────────────────────────────
run_scenario() {
    local scenario_file="$1"
    local scenario_id scenario_name

    scenario_id=$(basename "$scenario_file" .sh)
    scenario_name=$(head -3 "$scenario_file" | grep "^# " | head -1 | sed 's/^# //')

    echo ""
    log_header "${scenario_id}: ${scenario_name:-Unknown}"

    local log_file="${REPORT_DIR}/${scenario_id}.log"
    local start_ms end_ms
    start_ms=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")

    # Source the scenario — it should define:
    # - scenario_name
    # - SCENARIO_SCORE (0-10)
    # - SCENARIO_MAX_SCORE (default 10)
    # - SCENARIO_NOTES
    # - SCENARIO_STEPS (JSON array)
    SCENARIO_SCORE=0
    SCENARIO_MAX_SCORE=10
    SCENARIO_NOTES=""
    SCENARIO_STEPS="[]"

    set +e
    source "$scenario_file" 2>&1 | tee "$log_file"
    local source_exit=$?
    set -e

    end_ms=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
    SCENARIO_DURATION_MS=$((end_ms - start_ms))

    if [[ $source_exit -ne 0 ]]; then
        SCENARIO_NOTES="Scenario script failed with exit code ${source_exit}"
    fi

    write_result "$scenario_id" "${scenario_name:-$scenario_id}" \
        "${SCENARIO_SCORE:-0}" "${SCENARIO_MAX_SCORE:-10}" \
        "${SCENARIO_NOTES:-}" 

    # Append steps to result file
    if command -v jq &>/dev/null; then
        local tmp=$(jq --argjson steps "${SCENARIO_STEPS:-[]}" '.steps = $steps' "${REPORT_DIR}/${scenario_id}.result.json")
        echo "$tmp" > "${REPORT_DIR}/${scenario_id}.result.json"
    fi
}

# ─── Main ────────────────────────────────────────────────────────────────────
main() {
    log_header "Oxios Agent Experience Benchmark"
    echo -e "  Binary: ${BLUE}${OXIOS_BIN}${NC}"
    echo -e "  Home:   ${BLUE}${OXIOS_HOME}${NC}"
    echo -e "  Report: ${BLUE}${REPORT_DIR}${NC}"

    preflight

    # Discover scenarios
    local scenarios=()
    if [[ -n "$TARGET_SCENARIO" ]]; then
        local target="${SCENARIOS_DIR}/${TARGET_SCENARIO}.sh"
        if [[ -f "$target" ]]; then
            scenarios+=("$target")
        else
            log_error "Scenario not found: $target"
            exit 1
        fi
    else
        for f in "${SCENARIOS_DIR}"/s*.sh; do
            [[ -f "$f" ]] && scenarios+=("$f")
        done
    fi

    if [[ ${#scenarios[@]} -eq 0 ]]; then
        log_error "No scenarios found in ${SCENARIOS_DIR}/"
        exit 1
    fi

    log_info "Found ${#scenarios[@]} scenarios"

    # Run all scenarios
    for scenario in "${scenarios[@]}"; do
        run_scenario "$scenario"
    done

    # Generate report
    generate_report

    # Cleanup
    cleanup

    log_header "Benchmark Complete"
}

main "$@"
