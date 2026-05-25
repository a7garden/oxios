# RFC-009: Skill/Program 통합 — 단일 Skill 모델로 재설계

> **날짜**: 2026-05-25  
> **상태**: 초안  
> **범위**: `crates/oxios-kernel/src/skill.rs`, `crates/oxios-kernel/src/program/`, Web UI Skills/Programs/Host Tools 탭  
> **관련 이슈**: #2 (Host Tools 탭 무용함), #3 (Skill/Program 통합)

---

## 1. 배경

### 현재 문제

Oxios에는 **Skill**과 **Program**이라는 두 개념이 존재하는데, 실질적으로 같은 것입니다.

| 개념 | 파일 | 역할 |
|------|------|------|
| **Skill** | `SKILL.md` (YAML frontmatter) | 마크다운 지시문만 제공 |
| **Program** | `program.toml` + `SKILL.md` | 지시문 + 도구 정의 + 의존성 + MCP |

**"code-review Program"과 "code-review Skill"의 차이가 무엇인가요? 같은 것입니다.** Program이 Skill의 상위집합이라면, 둘을 나눌 이유가 없습니다.

### 구체적 문제점

1. **의존성 체크가 1차원** — `host_requirements`는 binaries만 체크. env, config 체크 없음
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
crates/oxios-kernel/src/tools/kernel_bridge.rs       — program_manager, skill_store
crates/oxios-kernel/src/kernel_handle/              — extension_api.rs
crates/oxios-kernel/src/capability/types.rs         — ResourceRef::Program
crates/oxios-kernel/src/capability/template.rs       — with_programs()
crates/oxios-kernel/src/config.rs                   — ExecConfig (host_tools)
crates/oxios-kernel/src/types.rs                    — SkillMeta
channels/oxios-web/web/src/routes/host-tools.tsx    — Host Tools 탭
channels/oxios-web/web/src/routes/programs.tsx      — Programs 탭
channels/oxios-web/web/src/routes/skills.tsx        — Skills 탭
```

---

## 2. 참고: OpenClaw 모델

OpenClaw는 **단일 Skill = Plugin = 도구 묶음** 모델을 사용합니다. 모든 메타데이터가 SKILL.md의 YAML frontmatter 안에 있습니다. 별도 TOML 파일 없음.

### 2.1 SKILL.md = Single Source of Truth

```yaml
---
name: code-review
description: Deep code review with quality domain analysis
author: oxios
version: 1.0.0
emoji: 🔍
homepage: https://...

requires:
  bins: ["git", "gh"]
  anyBins: ["ffmpeg", "avconv"]    # 하나라도 있으면 OK
  env: ["GITHUB_TOKEN"]
  config: ["github.pr.default-base"]
  os: ["darwin", "linux"]

install:
  - kind: brew
    formula: git
    os: ["darwin"]
  - kind: download
    url: https://...
    extract: true

always: false
user-invocable: true
disable-model-invocation: false
---

# Code Review

Instructions here...
```

### 2.2 Requirements = 5차원 의존성

```typescript
{
  requires: {
    bins: ["git", "gh"],           // 모두 필수
    anyBins: ["ffmpeg", "avconv"], // 하나라도 있으면 OK (OR instead of AND)
    env: ["GITHUB_TOKEN"],         // 환경변수
    config: ["github.pr.default-base"], // config 경로
    os: ["darwin", "linux"],       // OS
  }
}
```

### 2.3 Install Specs

```yaml
install:
  - kind: brew        # macOS homebrew
    formula: git
  - kind: node        # npm/pnpm/yarn/bun
    package: typescript
  - kind: go          # go install
    module: golang.org/x/tools/cmd/goimports
  - kind: uv          # Python uv
    package: black
  - kind: download    # HTTP download + extract
    url: https://...
    archive: .tar.gz
    extract: true
    stripComponents: 1
    os: ["linux"]     # OS 필터링
```

### 2.4 Skill Eligibility

```typescript
type SkillStatusEntry = {
  name: string;
  eligible: boolean;              // requirements 충족
  requirements: Requirements;
  missing: Requirements;          // 무엇이缺的
  configChecks: ConfigCheck[];
  install: SkillInstallOption[];  // 설치 가능한 옵션
  disabled: boolean;              // 사용자가 비활성화
  blockedByAllowlist: boolean;     // allowlist에 의한 차단
  always: boolean;                // requirements 무시
};
```

### 2.5 Per-Skill Config

`config.toml`에서 각 스킬별 설정을 제공:

```toml
[skills]
allowBundled = ["code-review", "debug"]   # bundled 스킬 허용 목록

[skills.entries.code-review]
enabled = true
env.GITHUB_TOKEN = "ghp_xxx"              # API key 오버라이드
config."github.pr.default-base" = "main"

[skills.entries.tts]
enabled = false
```

### 2.6 Skill Source Hierarchy (5단계)

```
workspace/.agents/<agent-id>/skills/   ← 에이전트 전용 스킬 (최고 우선순위)
workspace/skills/                     ← 프로젝트 스킬
plugins/*/skills/                    ← 플러그인 스킬
~/.oxios/workspace/skills/           ← 글로벌 사용자 스킬
share/skills/                        ← 번들 스킬 (oxios 기본 제공, 가장 낮음)
```

### 2.7 Skill Snapshot & Prompt 주입

```typescript
type SkillSnapshot = {
  prompt: string;           // formatSkillsForPrompt() 결과
  skills: Array<{
    name: string;
    primaryEnv?: string;
    requiredEnv?: string[];
  }>;
  skillFilter?: string[];   // 에이전트별 필터
  resolvedSkills?: Skill[];
};
```

프롬프트에 `<available_skills>` XML로 주입:

```xml
<available_skills>
  <skill>
    <name>code-review</name>
    <description>Deep code review...</description>
    <location>~/.oxios/skills/code-review/SKILL.md</location>
  </skill>
</available_skills>
```

### 2.8 File Watching

`chokidar`로 스킬 디렉토리를 감시. 변경 시 자동으로 snapshot을 갱신합니다.

---

## 3. 제안: 통합 Skill 모델

### 3.1 디렉토리 구조

```
~/.oxios/workspace/skills/
├── code-review/
│   └── SKILL.md              # frontmatter에 모든 메타데이터
├── debug/
│   └── SKILL.md
└── deploy/
    └── SKILL.md

share/skills/                   # 번들 스킬 (oxios 기본 제공)
├── code-review/
│   └── SKILL.md
└── guardian/
    └── SKILL.md
```

**별도 `skill.toml` 없음.** 모든 메타데이터가 SKILL.md frontmatter에 있습니다 (OpenClaw 모델).

### 3.2 SKILL.md Frontmatter 포맷

```yaml
---
name: code-review
description: Deep code review with quality domain analysis
author: oxios
version: 1.0.0
emoji: 🔍
homepage: https://github.com/...

# 4차원 Requirements (OpenClaw 모델)
requires:
  bins: ["git"]                 # 필수 바이너리
  anyBins: []                   # 하나라도 있으면 OK
  env: ["GITHUB_TOKEN"]         # 필수 환경변수
  config: []                    # 필수 config 경로

# OS 제한 (선택)
os: ["darwin", "linux"]

# 의존성 자동 설치
install:
  - kind: brew
    formula: git
    os: ["darwin"]
  - kind: download
    url: https://...

# 동작 제어
always: false                   # true면 requirements 무시하고 항상 eligible
user-invocable: true            # 사용자가 직접 호출 가능
disable-model-invocation: false  # model prompt에 포함 안 함
---

# Code Review

Instructions here...
```

### 3.3 Requirements 타입

```rust
/// 4차원 Requirements (OpenClaw 모델)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Requirements {
    /// Required binaries (all must be present).
    pub bins: Vec<String>,
    /// Alternative binaries (any one must be present).
    pub any_bins: Vec<String>,
    /// Required environment variables.
    pub env: Vec<String>,
    /// Required config paths.
    pub config: Vec<String>,
}

/// OS requirement.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OsRequirement {
    /// Allowed OS list.
    pub os: Vec<String>,
}

/// Install spec for automatic dependency installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInstallSpec {
    pub kind: InstallKind,
    pub formula: Option<String>,     // brew
    pub package: Option<String>,     // node/uv
    pub module: Option<String>,      // go
    pub url: Option<String>,        // download
    pub archive: Option<String>,
    pub extract: Option<bool>,
    pub strip_components: Option<u32>,
    pub target_dir: Option<String>,
    pub os: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InstallKind {
    Brew,
    Node,
    Go,
    Uv,
    Download,
}

/// Skill eligibility check result.
#[derive(Debug, Clone, Serialize)]
pub struct RequirementsCheck {
    pub missing_bins: Vec<String>,
    pub missing_any_bins: Vec<String>,
    pub missing_env: Vec<String>,
    pub missing_config: Vec<String>,
    pub missing_os: Vec<String>,
    pub eligible: bool,
    pub config_checks: Vec<ConfigCheck>,
}
```

### 3.4 SkillStatus

```rust
/// Skill eligibility status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillStatus {
    /// 모든 requirements 충족, 활성화됨.
    Ready,
    /// 일부 requirements 미충족.
    NeedsSetup,
    /// 사용자가 비활성화함.
    Disabled,
}
```

### 3.5 SkillEntry = 로드된 스킬 전체 상태

```rust
/// A loaded skill with full metadata and eligibility state.
#[derive(Debug, Clone)]
pub struct SkillEntry {
    /// The skill itself.
    pub skill: Skill,
    /// Parsed frontmatter (raw key-value map).
    pub frontmatter: HashMap<String, String>,
    /// Extended metadata parsed from frontmatter.
    pub metadata: Option<SkillMetadata>,
    /// Eligibility check result.
    pub eligibility: RequirementsCheck,
    /// Skill status derived from eligibility + config.
    pub status: SkillStatus,
    /// Whether this is a bundled skill.
    pub bundled: bool,
    /// Source scope (workspace, managed, bundled).
    pub source: SkillSource,
    /// Invocation policy.
    pub invocation: SkillInvocationPolicy,
}

/// Skill metadata from frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub author: Option<String>,
    pub version: Option<String>,
    pub emoji: Option<String>,
    pub homepage: Option<String>,
    pub requires: Requirements,
    pub os: Vec<String>,
    pub install: Vec<SkillInstallSpec>,
    pub always: bool,
    pub primary_env: Option<String>,
    pub skill_key: Option<String>,
}

/// Invocation policy from frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInvocationPolicy {
    pub user_invocable: bool,
    pub disable_model_invocation: bool,
}

/// Skill source scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillSource {
    /// Bundled with oxios (lowest priority).
    Bundled,
    /// User-managed global skills.
    Managed,
    /// Project workspace skills (highest priority).
    Workspace,
}
```

### 3.6 SkillManager API

```rust
/// Unified skill manager — replaces both SkillStore and ProgramManager.
pub struct SkillManager {
    skills_dir: PathBuf,                // ~/.oxios/workspace/skills/
    bundled_dir: PathBuf,                // share/skills/
    installed: RwLock<HashMap<String, SkillEntry>>,
    watcher: Option<FsWatcher>,
}

impl SkillManager {
    /// Create with workspace skills directory.
    pub fn new(skills_dir: PathBuf, bundled_dir: PathBuf) -> Self;

    /// Initialize: load all skills, start file watcher.
    pub async fn init(&self) -> Result<()>;

    /// List all skills with eligibility status.
    pub async fn list_skills(&self) -> Vec<SkillEntry>;

    /// Get a specific skill by name.
    pub async fn get_skill(&self, name: &str) -> Option<SkillEntry>;

    /// Check requirements for a specific skill.
    pub fn check_requirements(&self, skill: &Skill) -> RequirementsCheck;

    /// Get skill content for prompt injection.
    pub async fn get_skill_content(&self, name: &str) -> Option<String>;

    /// Build skills snapshot for an agent.
    pub async fn build_snapshot(
        &self,
        agent_id: Option<&str>,
        skill_filter: Option<&[String]>,
    ) -> SkillSnapshot;

    /// Enable or disable a skill.
    pub async fn set_enabled(&self, name: &str, enabled: bool) -> Result<()>;

    /// Get per-skill config overrides.
    pub fn get_skill_config(&self, name: &str) -> Option<SkillConfig>;

    /// Watch for file changes and auto-refresh.
    fn watch(&self) { ... }
}
```

### 3.7 Requirements 평가

```rust
impl Skill {
    /// Evaluate all requirements for this skill.
    pub fn check_requirements(&self, config: &OxiosConfig) -> RequirementsCheck {
        let has_bin = |bin: &str| which(bin).is_some();
        let is_env_satisfied = |env: &str| {
            std::env::var(env).is_ok()
                || config.skills.entries.get(&self.name).and_then(|c| c.env.get(env)).is_some()
        };
        let is_config_satisfied = |path: &str| {
            config.get_path(path).is_some()
                || config.skills.entries.get(&self.name).and_then(|c| c.config.get(path)).is_some()
        };

        let local_platform = if cfg!(target_os = "macos") {
            "darwin"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        };

        // bins: all must be present
        let missing_bins = self.metadata.as_ref()
            .map(|m| m.requires.bins.iter().filter(|b| !has_bin(b)).cloned().collect())
            .unwrap_or_default();

        // any_bins: at least one must be present
        let missing_any_bins = self.metadata.as_ref()
            .map(|m| {
                let req = &m.requires.any_bins;
                if req.is_empty() {
                    vec![]
                } else if req.iter().any(|b| has_bin(b)) {
                    vec![]
                } else {
                    req.clone()
                }
            })
            .unwrap_or_default();

        // env
        let missing_env = self.metadata.as_ref()
            .map(|m| m.requires.env.iter().filter(|e| !is_env_satisfied(e)).cloned().collect())
            .unwrap_or_default();

        // config
        let config_checks = self.metadata.as_ref()
            .map(|m| {
                m.requires.config.iter().map(|path| ConfigCheck {
                    path: path.clone(),
                    satisfied: is_config_satisfied(path),
                }).collect()
            })
            .unwrap_or_default();
        let missing_config = config_checks.iter().filter(|c| !c.satisfied).map(|c| c.path.clone()).collect();

        // os
        let missing_os = self.metadata.as_ref()
            .map(|m| {
                if m.os.is_empty() || m.os.iter().any(|o| o == local_platform) {
                    vec![]
                } else {
                    m.os.clone()
                }
            })
            .unwrap_or_default();

        let eligible = self.metadata.as_ref()
            .map(|m| m.always)
            .unwrap_or(false)
            || (missing_bins.is_empty()
                && missing_any_bins.is_empty()
                && missing_env.is_empty()
                && missing_config.is_empty()
                && missing_os.is_empty());

        RequirementsCheck {
            missing_bins,
            missing_any_bins,
            missing_env,
            missing_config,
            missing_os,
            eligible,
            config_checks,
        }
    }
}
```

### 3.8 SkillSnapshot = 프롬프트 주입용

```rust
/// Snapshot of resolved skills for an agent run.
/// Built once per agent run and embedded in the system prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSnapshot {
    /// Formatted XML prompt block for skills.
    pub prompt: String,
    /// All skills the agent can see (model-visible).
    pub skills: Vec<SkillRef>,
    /// Skill filter used to build this snapshot.
    pub skill_filter: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRef {
    pub name: String,
    pub description: String,
    pub file_path: String,
    pub primary_env: Option<String>,
    pub required_env: Vec<String>,
}

impl SkillManager {
    /// Build a skill snapshot for agent initialization.
    pub async fn build_snapshot(
        &self,
        agent_id: Option<&str>,
        skill_filter: Option<&[String]>,
    ) -> SkillSnapshot {
        let entries = self.list_skills().await;

        // Filter by eligibility + allowlist + skill filter
        let visible: Vec<_> = entries.iter()
            .filter(|e| {
                e.status != SkillStatus::Disabled
                && e.eligibility.eligible
                && e.invocation.disable_model_invocation == false
            })
            .collect();

        // Apply agent skill filter if provided
        let filtered: Vec<_> = if let Some(filter) = skill_filter {
            visible.iter().filter(|e| filter.contains(&e.skill.name)).cloned().collect()
        } else {
            visible
        };

        // Build prompt
        let prompt = format_skills_for_prompt(&filtered);
        let skills = filtered.iter().map(|e| SkillRef {
            name: e.skill.name.clone(),
            description: e.skill.description.clone(),
            file_path: e.skill.file_path.to_string_lossy().into_owned(),
            primary_env: e.metadata.as_ref().and_then(|m| m.primary_env.clone()),
            required_env: e.metadata.as_ref().map(|m| m.requires.env.clone()).unwrap_or_default(),
        }).collect();

        SkillSnapshot { prompt, skills, skill_filter: skill_filter.map(|f| f.to_vec()) }
    }
}

/// Format skills as XML prompt block (matches OpenClaw output format).
fn format_skills_for_prompt(skills: &[&SkillEntry]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut lines = vec![
        "\n\nThe following skills provide specialized instructions for specific tasks.",
        "Use the read tool to load a skill's file when the task matches its description.",
        "When a skill file references a relative path, resolve it against the skill directory (parent of SKILL.md / dirname of the path) and use that absolute path in tool commands.",
        "",
        "<available_skills>",
    ];
    for skill in skills {
        lines.push("  <skill>");
        lines.push(&format!("    <name>{}</name>", escape_xml(&skill.skill.name)));
        lines.push(&format!("    <description>{}</description>", escape_xml(&skill.skill.description)));
        lines.push(&format!("    <location>{}</location>", escape_xml(&skill.skill.file_path.to_string_lossy())));
        lines.push("  </skill>");
    }
    lines.push("</available_skills>");
    lines.join("\n")
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
```

---

## 4. 제거되는 것들

| 제거 대상 | 대체 |
|---------|------|
| `SkillStore` | `SkillManager` (단일化) |
| `ProgramManager` | `SkillManager` (병합) |
| `program/` 모듈 전체 | `skill/`으로 통합 |
| `HostToolValidator` | `Skill::check_requirements()` |
| `ExecConfig.required_host_tools` | 각 스킬의 `requires.bins` |
| `ExecConfig.optional_host_tools` | 각 스킬의 `requires.optional_bins` → `requires.any_bins` |
| `ResourceRef::Program` | `ResourceRef::Skill` |
| `CapabilityTemplate::with_programs()` | `with_skills()` |
| `ExtensionApi::program_manager` | `ExtensionApi::skill_manager` |
| `Host Tools 탭` | Skills 탭에 requirements 표시로 흡수 |
| `Programs 탭` | Skills 탭에 통합 |
| `.programs/` 디렉토리 | `share/skills/` 로 교체 |
| `skill.toml` 포맷 | SKILL.md frontmatter (OpenClaw 모델) |

---

## 5. API 변경

### 5.1 `/api/skills` (변경)

```json
{
  "skills": [
    {
      "name": "code-review",
      "description": "Deep code review...",
      "author": "oxios",
      "version": "1.0.0",
      "emoji": "🔍",
      "homepage": "https://...",
      "source": "managed",
      "bundled": false,
      "status": "ready",
      "eligible": true,
      "always": false,
      "user_invocable": true,
      "file_path": "~/.oxios/skills/code-review/SKILL.md",
      "requirements": {
        "bins": ["git"],
        "any_bins": [],
        "env": ["GITHUB_TOKEN"],
        "config": []
      },
      "missing": {
        "bins": [],
        "any_bins": [],
        "env": [],
        "config": []
      },
      "os": ["darwin", "linux"],
      "install": [
        { "kind": "brew", "label": "Install git (brew)", "bins": ["git"] }
      ],
      "config_checks": []
    }
  ]
}
```

### 5.2 `/api/skills/:name/enable` (변경)
### 5.3 `/api/skills/:name/disable` (변경)
### 5.4 `/api/skills/:name/content` (변경)

### 5.5 `/api/programs` → deprecated (삭제 예정)

호환성 유지를 위해 응답을 `/api/skills`로 리다이렉션. 클라이언트에 deprecation 경고 포함.

### 5.6 `/api/host-tools` → deprecated (삭제 예정)

호환성 유지를 위해 빈 배열 반환. 클라이언트에 deprecation 경고 포함.

---

## 6. UI 변경

### 6.1 Skills 탭 (새로 설계)

```
Skills
┌──────────────────────────────────────────────────────┐
│ 🔍 Filter: [All] [Ready] [Needs Setup] [Disabled]    │
├──────────────────────────────────────────────────────┤
│ ┌──────────────────────────────────────────────────┐ │
│ │ 🔍 code-review                        🟢 ready    │ │
│ │    Deep code review...                 v1.0.0      │ │
│ │    managed                                    │ │
│ │                                                │ │
│ │    requires                                   │ │
│ │      bins     git ✅                           │ │
│ │      env      GITHUB_TOKEN ✅                 │ │
│ │                                                │ │
│ │    install                                    │ │
│ │      Install git (brew)                        │ │
│ └──────────────────────────────────────────────────┘ │
│ ┌──────────────────────────────────────────────────┐ │
│ │ 🐛 debug                              🟡 needs-setup │ │
│ │    Debug Rust/C++ programs          v1.0.0      │ │
│ │    bundled                                   │ │
│ │                                                │ │
│ │    requires                                   │ │
│ │      bins     git ✅, lldb ❌ (missing)         │ │
│ │      any_bins  cargo ✅                        │ │
│ │                                                │ │
│ │    install                                    │ │
│ │      Install lldb (brew)                      │ │
│ │      Install rust (brew)                       │ │
│ └──────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

### 6.2 사이드바

- ~~Programs~~ → 제거
- ~~Host Tools~~ → 제거
- **Skills** → 통합된 Skills 탭으로 이동

---

## 7. 구현 단계

### Phase 1: 타입 정의 + Frontmatter 파서 (1-2일)

- [ ] `skill.rs` 리뉴얼: `Requirements`, `SkillMetadata`, `SkillEntry`, `SkillStatus`, `SkillInvocationPolicy`, `SkillSource`, `RequirementsCheck`, `ConfigCheck`, `SkillInstallSpec`, `InstallKind`
- [ ] frontmatter 파서 확장: `requires.bins/env/config`, `install`, `os`, `always`, `user-invocable`, `disable-model-invocation`, `emoji`, `homepage`, `author`, `version`, `primary-env`
- [ ] `format_skills_for_prompt()` 구현 (XML 포맷)

### Phase 2: SkillManager 구현 (2-3일)

- [ ] `SkillManager` 구조체 정의 (`skills_dir`, `bundled_dir`, `installed`, `watcher`)
- [ ] `SkillManager::init()` — bundled 로드 + workspace 로드 + 파일 감시 시작
- [ ] `Skill::check_requirements()` 구현
- [ ] `list_skills()`, `get_skill()`, `get_skill_content()` 구현
- [ ] `build_snapshot()` 구현
- [ ] `set_enabled()` 구현 (state.json 유지)
- [ ] `ProgramManager`의 upgrade 로직 포팅 (enabled state 보존)

### Phase 3: 기존 모듈 정리 (1일)

- [ ] `kernel_bridge.rs` — `program_manager` → `skill_manager`, `skill_store` 제거
- [ ] `extension_api.rs` — `program_manager` → `skill_manager`, program 관련 API 제거
- [ ] `capability/types.rs` — `ResourceRef::Program` → `ResourceRef::Skill`
- [ ] `capability/template.rs` — `with_programs()` → `with_skills()`
- [ ] `host_tools.rs` 제거 또는 `HostToolValidator` → `Skill::check_requirements()`
- [ ] `config.rs` — `ExecConfig.required_host_tools` / `optional_host_tools` 제거

### Phase 4: API 업데이트 (1일)

- [ ] `/api/skills` 응답 변경 (requirements, status, missing 추가)
- [ ] `/api/programs` → `/api/skills` 리다이렉션
- [ ] `/api/host-tools` → deprecated 응답

### Phase 5: Web UI (2-3일)

- [ ] Skills 탭 재설계 (requirements, status, install 옵션 표시)
- [ ] Programs 탭 제거
- [ ] Host Tools 탭 제거
- [ ] 사이드바 업데이트
- [ ] `types/index.ts` 업데이트

### Phase 6: 번들 스킬 마이그레이션 (1일)

- [ ] `.programs/code-review/` → `share/skills/code-review/SKILL.md` 변환
- [ ] `program.toml` frontmatter → SKILL.md frontmatter 변환
- [ ] `.programs/` 제거 (또는 share/로 이동)

### Phase 7: 테스트 + 문서 (1일)

- [ ] 기존 테스트 업데이트
- [ ] AGENTS.md 업데이트
- [ ] `docs/channel-plugin-guide.md` 업데이트 (필요 시)

---

## 8. 체크리스트

### Backend

- [ ] `Requirements`, `SkillMetadata`, `SkillEntry` 타입 정의
- [ ] frontmatter 파서 확장 (OpenClaw 포맷)
- [ ] `Skill::check_requirements()` 구현 (bins, any_bins, env, config, os)
- [ ] `SkillManager` 구현 (init, list, get, snapshot, watch)
- [ ] `SkillManager::build_snapshot()` + `format_skills_for_prompt()`
- [ ] `SkillManager::set_enabled()` (state.json)
- [ ] `kernel_bridge.rs` 업데이트
- [ ] `extension_api.rs` 업데이트
- [ ] `capability/` 업데이트
- [ ] `program/` 모듈 제거
- [ ] `host_tools.rs` 제거
- [ ] `config.rs` 정리
- [ ] API 업데이트 (`/api/skills`)
- [ ] 기존 테스트 업데이트

### Frontend

- [ ] Skills 탭 재설계
- [ ] Programs 탭 제거
- [ ] Host Tools 탭 제거
- [ ] 사이드바 업데이트
- [ ] `bun run build` 통과
- [ ] `tsc --noEmit` 통과

### Docs

- [ ] AGENTS.md 업데이트
- [ ] `docs/program-development.md` → `docs/skill-development.md` (또는 삭제)

---

## 9. 리스크

| 리스크 | 대응 |
|--------|------|
| 기존 `.programs/` 스킬이 사라짐 | share/skills/로 자동 복사 (Phase 6) |
| frontmatter 파싱 에러 |graceful degradation — 파싱 실패 시 skill만 유지 |
| Requirements 평가 오버헤드 | `which()` 결과 캐싱, startup 시 1회 평가 |
| config 체크 보안 노출 | 값이 아닌 존재 여부만 체크 |
| 파일 감시 메모리 누수 | 구독자가 0이면 watcher 해제 |