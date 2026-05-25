# RFC-009: Skill/Program 통합 — 단일 Skill 모델로 재설계

> **날짜**: 2026-05-25
> **상태**: 초안
> **범위**: `crates/oxios-kernel/src/skill.rs`, `crates/oxios-kernel/src/program/`, `crates/oxios-kernel/src/host_tools.rs`, Web UI Skills/Programs/Host Tools 탭
> **관련 이슈**: #2 (Host Tools 탭 무용함)

---

## 1. 배경

### 현재 문제

Oxios에는 **Skill**과 **Program**이라는 두 개념이 존재하는데, 실질적으로 같은 것입니다.

| 개념 | 파일 | 역할 |
|------|------|------|
| **Skill** | `SKILL.md` (YAML frontmatter) | 마크다운 지시문만 제공 |
| **Program** | `program.toml` + `SKILL.md` | 지시문 + 도구 정의 + 의존성 + MCP |

"code-review Program"과 "code-review Skill"의 차이가 무엇인가요? 같은 것입니다. Program이 Skill의 상위집합이라면, 둘을 나눌 이유가 없습니다.

### 구체적 문제점

1. **의존성 체크가 1차원** — `host_requirements`는 binaries만 체크. env, config, auth 체크 없음
2. **Host Tools 탭이 무용** — `config.toml`의 글로벌 설정만 보여주는데, 기본값이 빈 배열
3. **Host Tools ≠ ExecTool allowlist** — 서로 다른 config, 서로 다른 체크 로직
4. **Program이 선언한 의존성이 UI에 안 보임** — Programs 탭은 이름/버전만 표시
5. **Skill은 사실상 Program의 부분집합** — 메타데이터 + 마크다운뿐, 도구/의존성 없음

### 영향받는 코드

```
crates/oxios-kernel/src/skill.rs                    — SkillStore (94줄)
crates/oxios-kernel/src/program/                    — ProgramManager (전체 모듈)
crates/oxios-kernel/src/host_tools.rs               — HostToolValidator
crates/oxios-kernel/src/tools/exec_tool.rs          — ExecTool (allowlist)
crates/oxios-kernel/src/config.rs                   — ExecConfig (host_tools)
channels/oxios-web/web/src/routes/host-tools.tsx    — Host Tools 탭
channels/oxios-web/web/src/routes/programs.tsx      — Programs 탭
channels/oxios-web/web/src/routes/skills.tsx        — Skills 탭
```

---

## 2. 참고: OpenClaw 모델

OpenClaw는 **단일 Skill = Plugin = 도구 묶음** 모델을 사용합니다.

### 의존성 선언

각 스킬이 **4차원 requirements**를 직접 선언합니다:

```typescript
{
  requirements: {
    bins: ["python3"],              // CLI 바이너리
    anyBins: ["ffmpeg", "avconv"],  // 하나라도 있으면 OK
    env: ["OPENAI_API_KEY"],        // 환경변수
    config: ["media.image.provider"], // 설정 경로
    os: ["darwin", "linux"],        // OS
  }
}
```

### 실시간 평가

스킬 로딩 시 의존성을 평가하여 `eligible` / `blocked` 상태를 결정합니다:

```typescript
function evaluateRequirements() {
  missing.bins    → hasLocalBin("git")       // which git
  missing.env     → isEnvSatisfied("API_KEY") // process.env
  missing.config  → isConfigSatisfied(...)    // config 경로
  missing.os      → localPlatform check

  eligible = missing이 전부 비어있음
}
```

### Tool Availability

각 도구가 **선언적 availability**를 가집니다:

```typescript
{
  name: "web_search",
  availability: {
    kind: "config",
    path: ["web-search", "provider"],
    check: "exists"
  }
}
```

auth, config, env, plugin-enabled, context 등 다양한 시그널을 지원합니다.

### UI

Skills 탭에서 각 스킬의 상태를 실시간으로 보여줍니다:

```
Filter: [All] [Ready] [Needs Setup] [Disabled]

code-review      🟢 ready
debug            🟡 needs-setup    missing: bin:lldb
deploy           🟢 ready
tts              🟡 needs-setup    missing: env:OPENAI_API_KEY
```

---

## 3. 제안: 통합 Skill 모델

### 3.1 디렉토리 구조

```
~/.oxios/workspace/skills/
├── code-review/
│   ├── skill.toml       # 메타데이터 + requirements + tools
│   ├── SKILL.md         # 에이전트 지시문
│   └── bin/             # (선택) 실행 파일
├── debug/
│   ├── skill.toml
│   └── SKILL.md
└── deploy/
    ├── skill.toml
    └── SKILL.md
```

### 3.2 skill.toml 포맷

```toml
[skill]
name = "code-review"
version = "1.0.0"
description = "Deep code review with quality domain analysis"
author = "oxios"

[requirements]
bins = ["git"]                           # 필수 CLI 바이너리
env = []                                 # 필수 환경변수
config = []                               # 필수 config.toml 경로

[tools]
read = "Read file contents"
exec = "Execute commands"
grep = "Search file contents"
find = "Find files by name"

[mcp]                                     # (선택) MCP 서버
name = "code-review-mcp"
command = "node"
args = ["mcp-server.js"]
```

기존 `program.toml`과의 차이:
- `[program]` → `[skill]` (이름만 변경)
- `[host_requirements]` → `[requirements]` (확장)
- `bins`만 → `bins` + `env` + `config` (3차원)
- `SKILL.md`는 그대로 유지

### 3.3 Requirements 평가

```rust
/// Skill requirements evaluation result.
pub struct RequirementsCheck {
    /// Missing binaries.
    pub missing_bins: Vec<String>,
    /// Missing environment variables.
    pub missing_env: Vec<String>,
    /// Missing config paths.
    pub missing_config: Vec<String>,
    /// Whether all requirements are satisfied.
    pub eligible: bool,
}

impl Skill {
    pub fn check_requirements(&self) -> RequirementsCheck {
        let missing_bins = self.requirements.bins.iter()
            .filter(|bin| !which(bin))
            .cloned()
            .collect();

        let missing_env = self.requirements.env.iter()
            .filter(|var| std::env::var(var).is_err())
            .cloned()
            .collect();

        let missing_config = self.requirements.config.iter()
            .filter(|path| !config_path_exists(path))
            .cloned()
            .collect();

        let eligible = missing_bins.is_empty()
            && missing_env.is_empty()
            && missing_config.is_empty();

        RequirementsCheck { missing_bins, missing_env, missing_config, eligible }
    }
}
```

### 3.4 SkillStatus

```rust
pub enum SkillStatus {
    /// 모든 requirements 충족, 활성화됨.
    Ready,
    /// 일부 requirements 미충족.
    NeedsSetup,
    /// 사용자가 비활성화함.
    Disabled,
}
```

### 3.5 제거되는 것들

| 제거 | 대체 |
|------|------|
| `ProgramManager` | `SkillManager` (통합) |
| `SkillStore` | `SkillManager` (통합) |
| `HostToolValidator` | `Skill::check_requirements()` |
| `ExecConfig.required_host_tools` | 각 Skill의 `[requirements]` |
| `ExecConfig.optional_host_tools` | 각 Skill의 `[requirements]` |
| Host Tools 탭 | Skills 탭에 requirements 표시로 흡수 |
| Programs 탭 | Skills 탭에 통합 |
| `.programs/` 디렉토리 | `skills/`에 통합 |

### 3.6 UI 변경

Skills 탭 하나로 통합:

```
Skills
┌──────────────────────────────────────────────────────┐
│ code-review      🟢 ready                v1.0.0      │
│   requires: git ✅                                    │
│   tools: read, exec, grep, find                       │
├──────────────────────────────────────────────────────┤
│ debug            🟡 needs-setup          v1.0.0      │
│   requires: git ✅, lldb ❌                           │
│   tools: read, exec                                   │
├──────────────────────────────────────────────────────┤
│ deploy           🟢 ready                v1.0.0      │
│   requires: gh ✅, kubectl ✅                         │
│   tools: exec                                         │
├──────────────────────────────────────────────────────┤
│ guardian          🟢 ready                v1.0.0      │
│   (no requirements)                                    │
└──────────────────────────────────────────────────────┘

Filter: [All] [Ready] [Needs Setup] [Disabled]
```

사이드바에서 제거:
- ~~Programs~~ → Skills에 흡수
- ~~Host Tools~~ → Skills에 흡수

---

## 4. 마이그레이션 계획

### Phase 1: 타입 통합 (Breaking)

1. `ProgramMeta` + `SkillMeta` → 통합 `SkillMeta` (skill.toml 기반)
2. `ProgramHostRequirements` → `Requirements` (bins + env + config)
3. `Program` + `Skill` → 통합 `Skill`
4. `SkillStore` + `ProgramManager` → `SkillManager`

### Phase 2: 의존성 평가 시스템

1. `RequirementsCheck` 구현 (bins, env, config)
2. `SkillManager::list_with_status()` — 각 스킬의 eligible/blocked 상태 반환
3. `HostToolValidator` 제거, `Skill::check_requirements()`로 대체

### Phase 3: API 업데이트

1. `GET /api/skills` → `requirements`, `status`, `missing` 필드 추가
2. `GET /api/programs` → deprecated, `/api/skills`로 리다이렉트
3. `GET /api/host-tools` → deprecated, `/api/skills`의 requirements로 대체
4. `POST /api/skills/:name/install` — 의존성 자동 설치 (future)

### Phase 4: UI 통합

1. Skills 탭에 requirements 표시
2. Programs 탭 제거
3. Host Tools 탭 제거
4. 사이드바 업데이트
5. Filter (All / Ready / Needs Setup / Disabled)

### Phase 5: 기존 .programs/ 마이그레이션

1. 시작 시 `.programs/*/program.toml` → `skills/*/skill.toml` 자동 변환
2. `[program]` → `[skill]`
3. `[host_requirements]` → `[requirements]`
4. 마이그레이션 로그 출력

---

## 5. 체크리스트

- [ ] `SkillMeta` 통합 타입 정의
- [ ] `skill.toml` 파서 구현
- [ ] `Requirements` 타입 (bins, env, config)
- [ ] `Skill::check_requirements()` 구현
- [ ] `SkillManager` 통합 구현
- [ ] `ProgramManager` / `SkillStore` 제거
- [ ] `HostToolValidator` 제거
- [ ] API 업데이트 (`/api/skills`)
- [ ] UI Skills 탭 재설계
- [ ] UI Programs 탭 제거
- [ ] UI Host Tools 탭 제거
- [ ] 사이드바 업데이트
- [ ] `.programs/` → `skills/` 마이그레이션 로직
- [ ] 기존 테스트 업데이트
- [ ] AGENTS.md 업데이트

---

## 6. 리스크

| 리스크 | 대응 |
|--------|------|
| 기존 `.programs/` 사용자 영향 | 자동 마이그레이션 (Phase 5) |
| Skill/Program 분리를 의존하는 외부 코드 | `/api/programs`를 deprecated로 유지하다가 제거 |
| bins 체크의 `which` 오버헤드 | 캐싱 + 시작 시 1회 평가 |
| env/config 체크의 보안 노출 | 값이 아닌 존재 여부만 체크 |
