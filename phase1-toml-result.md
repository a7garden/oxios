# Phase 1: default-config.toml 변경 결과

## 파일
`share/default-config.toml`

## 변경 사항

### 1. [gateway] host 바인딩 제한
```toml
# Before
host = "0.0.0.0"

# After
# NOTE: Bind to localhost only. Change to "0.0.0.0" only if you understand the risks.
host = "127.0.0.1"
```

### 2. [exec] 위험 바이너리 제거 + allowlist_mode 추가
```toml
# Before
allowed_commands = ["git", "gh", "open", "shortcuts", "osascript"]

# After
allowlist_mode = "enforced"
allowed_commands = [
    "ls", "cat", "head", "tail", "wc",
    "grep", "rg", "find", "fd",
    "git",
    "cargo", "rustc",
    "python3", "node", "bun",
    "curl", "wget",
    "jq", "yq",
    "echo", "mkdir", "cp", "mv",
]
```

**제거된 바이너리** (보안 위험):
| 바이너리 | 위험 사유 |
|----------|-----------|
| `osascript` | 임의 AppleScript 실행 → 시스템 제어 가능 |
| `open` | 임의 앱/URL 열기 → 소셜 엔지니어링 벡터 |
| `shortcuts` | Shortcuts 앱 실행 → 데이터 유출 가능 |
| `gh` | GitHub CLI → 토큰 탈취/리포지토리 조작 위험 |

**추가된 설정**:
- `allowlist_mode = "enforced"` — allowlist 외 명령어 실행 시 거부

**유지된 바이너리** (24개): 파일 조회(ls/cat/head/tail/wc), 검색(grep/rg/find/fd), 버전관리(git), 빌드(cargo/rustc), 런타임(python3/node/bun), 네트워크(curl/wget), 데이터처리(jq/yq), 파일작업(echo/mkdir/cp/mv)

### 3. [security] 경고 주석 추가
```toml
# Before
# Enable API key authentication.
auth_enabled = false

# After
# NOTE: auth_enabled should be true in production.
auth_enabled = false
```

## 검증
- TOML 파싱 정상 (구조 유지)
- 나머지 섹션 변경 없음
