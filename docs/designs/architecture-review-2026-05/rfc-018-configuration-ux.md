# RFC-018: 설정 UX 개선

> **상태:** 📝 설계 (v2 — 분석 기반 개정)
> **날짜:** 2026-05-27
> **우선순위:** P1 (Phase 0) / P2 (Phase 1–4)
> **범위:** `src/main.rs`, `crates/oxios-kernel/src/config.rs`, `share/default-config.toml`, `AGENTS.md`
> **선행:** 없음
> **후행:** 없음

---

## 1. 동기

### 1.1 현재 문제

코드베이스 교차 검증 결과:

| # | 문제 | 심각도 | 검증 |
|---|------|--------|------|
| 1 | `config set`이 **161개** 필드 중 9개만 지원 | 🔴 | `main.rs:426-504` match arm 9개 |
| 2 | `config set`이 `toml::to_string_pretty()`로 전체 재직렬화 → **사용자 코멘트/포맷팅 전부 파괴** | 🔴 | `main.rs:453-454` |
| 3 | `max_agents` 기본값 불일치: TOML(`default-config.toml:11`)=10, Rust(`config.rs:762`)=16 | 🟡 | 실제 불일치 확인 |
| 4 | AGENTS.md에 포트 3000, 실제는 4200 (`vite.config.ts:32` 확인) | 🟢 | 3곳 (88, 317, 324행) |
| 5 | `memory.consolidation` **29개** 필드에 프리셋 없음 | 🟡 | config.rs ConsolidationConfig 29개 pub 필드 |
| 6 | `oxios pkg install` (미구현) vs `oxios marketplace install` (작동) 혼란 | 🟢 | 두 개의 독립적 서브명령어 존재 |
| 7 | `engine.default_model` 기본값 빈 문자열 — 에러 메시지 불친절 | 🟡 | `Default`가 `String::new()` 반환 |
| 8 | `WORKSPACE_SUBDIRS`가 `main.rs:298`과 `onboarding.rs:31`에 각각 정의 | 🟢 | 완전 동일 배열 |
| 9 | `config list` / `config describe` / `config reset` 부재 | 🟡 | ConfigAction에 Show/Set/Get만 존재 |

**제외 (v1 오류 수정):** 기존 "gateway.host 불일치: TOML=0.0.0.0, Rust=127.0.0.1"은 **사실 무근**. 둘 다 `127.0.0.1`로 통일되어 있음.

### 1.2 가장 심각한 문제: 코멘트 파괴

`default-config.toml`은 166행의 주석이 풍부한 파일. 사용자가 이를 기반으로 config를 수정한 뒤
`oxios config set gateway.port 8080`을 실행하면, **모든 코멘트가 날아간 73행 bare TOML**으로 변합니다.

```rust
// 현재 main.rs:453-454 — 전체 재직렬화
let toml_str = toml::to_string_pretty(&config)?;  // ← 코멘트 전부 소멸
std::fs::write(config_path, toml_str)?;
```

이것은 사용자 신뢰를 직접적으로 훼솑하는 버그입니다.

---

## 2. 설계

### 2.1 `toml_edit` 기반 Config Set (코멘트 보존)

현재 `toml::to_string_pretty()` 전체 재직렬화를 `toml_edit` 부분 수정으로 교체:

**의존성 추가:** `toml_edit = "0.22"` (바이너리 `Cargo.toml`)

```rust
// main.rs — set 경로를 toml_edit 기반으로 교체

fn cmd_config_set(config_path: &Path, key: &str, raw_value: &str) -> Result<()> {
    // 1. 기존 TOML 읽기 (코멘트/포맷팅 보존)
    let toml_str = std::fs::read_to_string(config_path)
        .context("설정 파일을 읽을 수 없습니다")?;
    let mut doc = toml_str.parse::<toml_edit::DocumentMut>()
        .context("설정 파일 파싱 실패")?;

    // 2. 값 타입 추론 — 기존 필드 타입 존중
    let existing_type = get_existing_type(&doc, key);
    let parsed = parse_toml_value(raw_value, existing_type);

    // 3. dot-notation으로 테이블 탐색 + leaf 값 설정
    set_toml_dot(&mut doc, key, parsed)?;

    // 4. 쓰기 (코멘트 보존)
    std::fs::write(config_path, doc.to_string())?;

    info!("설정 변경: {} = {}", key, raw_value);
    Ok(())
}

fn set_toml_dot(doc: &mut DocumentMut, key: &str, value: toml_edit::Item) -> Result<()> {
    let parts: Vec<&str> = key.split('.').collect();
    let mut table = doc.as_table_mut();

    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            table[*part] = value;
        } else {
            table = table.entry(*part)
                .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
                .as_table_mut()
                .ok_or_else(|| anyhow!("'{}'는 테이블이 아닙니다", parts[..=i].join(".")))?;
        }
    }
    Ok(())
}
```

**핵심 차이:** `parse_toml_value`가 기존 필드 타입을 존중:

```rust
enum ExistingType { Bool, Integer, Float, String, Unknown }

fn get_existing_type(doc: &DocumentMut, key: &str) -> ExistingType {
    let parts: Vec<&str> = key.split('.').collect();
    let mut table = doc.as_table();
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            return match table.get(*part) {
                Some(Item::Value(Value::from(v))) if v.is_bool() => ExistingType::Bool,
                Some(Item::Value(Value::from(v))) if v.is_integer() => ExistingType::Integer,
                Some(Item::Value(Value::from(v))) if v.is_float() => ExistingType::Float,
                Some(Item::Value(_)) => ExistingType::String,
                _ => ExistingType::Unknown,
            };
        }
        table = match table.get(*part).and_then(|t| t.as_table()) {
            Some(t) => t,
            None => return ExistingType::Unknown,
        };
    }
    ExistingType::Unknown
}

fn parse_toml_value(raw: &str, existing: ExistingType) -> toml_edit::Item {
    // 기존 타입을 존중 — "true"가 String 필드에 들어가면 String 유지
    match existing {
        ExistingType::Bool => {
            let v = raw.parse::<bool>()
                .unwrap_or_else(|_| panic!("boolean 값이 필요합니다 (true/false): {}", raw));
            return toml_edit::value(v);
        }
        ExistingType::Integer => {
            if let Ok(n) = raw.parse::<i64>() { return toml_edit::value(n); }
        }
        ExistingType::Float => {
            if let Ok(n) = raw.parse::<f64>() { return toml_edit::value(n); }
        }
        ExistingType::String | ExistingType::Unknown => {}
    }
    // Unknown: 자동 추론 (기존 동작 보존)
    if raw == "true" { return toml_edit::value(true); }
    if raw == "false" { return toml_edit::value(false); }
    if let Ok(n) = raw.parse::<i64>() { return toml_edit::value(n); }
    if let Ok(n) = raw.parse::<f64>() { return toml_edit::value(n); }
    toml_edit::value(raw)
}
```

### 2.2 Dot-notation Config Get

`serde_json::to_value()` + `pointer()`로 전체 필드 조회:

```rust
fn get_config_value(config: &OxiosConfig, key: &str) -> Result<String> {
    let json = serde_json::to_value(config)
        .context("설정을 JSON으로 변환 실패")?;

    let value = json.pointer(&format!("/{}", key.replace('.', "/")))
        .ok_or_else(|| anyhow!(
            "알 수 없는 설정 키: '{}'\n\
             사용 가능한 키는 `oxios config list`로 확인하세요.",
            key
        ))?;

    match value {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Null => Ok("null".to_string()),
        Value::Array(_) | Value::Object(_) => {
            Ok(serde_json::to_string_pretty(value)?)
        }
    }
}
```

### 2.3 추가 명령어: `list`, `reset`

```rust
#[derive(Debug, Clone, Subcommand)]
enum ConfigAction {
    /// 전체 설정 출력
    Show,

    /// 설정값 조회
    Get { key: String },

    /// 설정값 변경 (코멘트/포맷팅 보존)
    Set { key: String, value: String },

    /// 모든 설정 키 나열
    List {
        /// 필터 접두어 (예: "memory" → memory.* 만 표시)
        prefix: Option<String>,
    },

    /// 설정값을 기본값으로 되돌림
    Reset { key: String },
}
```

`list` 구현:

```rust
fn cmd_config_list(config: &OxiosConfig, prefix: Option<&str>) -> Result<()> {
    let json = serde_json::to_value(config)?;
    let prefix_path = prefix
        .map(|p| format!("/{}", p.replace('.', "/")))
        .unwrap_or_default();

    let root = json.pointer(&prefix_path)
        .ok_or_else(|| anyhow!("알 수 없는 접두어: '{}'", prefix.unwrap_or_default()))?;

    let mut keys = Vec::new();
    collect_leaf_keys(root, prefix.unwrap_or(""), &mut keys);

    if keys.is_empty() {
        println!("  설정 키가 없습니다.");
    } else {
        for (key, value) in keys {
            println!("  {:<50} {}", key, style(value).dim());
        }
        println!();
        println!("  {}개 설정 키", style(keys.len()).cyan());
    }
    Ok(())
}

fn collect_leaf_keys(value: &Value, prefix: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let new_prefix = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
                collect_leaf_keys(v, &new_prefix, out);
            }
        }
        _ => {
            let display = match value {
                Value::String(s) => format!("\"{}\"", s),
                Value::Null => "null".into(),
                other => other.to_string(),
            };
            out.push((prefix.to_string(), display));
        }
    }
}
```

`reset` 구현:

```rust
fn cmd_config_reset(config_path: &Path, key: &str) -> Result<()> {
    let defaults = OxiosConfig::default();
    let default_value = get_config_value(&defaults, key)
        .map_err(|_| anyhow!("알 수 없는 설정 키: '{}'", key))?;

    cmd_config_set(config_path, key, &default_value)?;
    println!("  {} {} → 기본값 ({})", style("Reset").green(), key, default_value);
    Ok(())
}
```

### 2.4 기본값 통일 — 단일 진실 공급원

Rust `Default` impl을 단일 진실 공급원으로 유지. `default-config.toml`은 **템플릿**이지 소스가 아님.

```rust
// config.rs — max_agents 기본값을 10으로 통일
fn default_max_agents() -> usize {
    10  // ← 기존 16에서 변경
}
```

**회귀 방지 테스트 추가:**

```rust
#[test]
fn test_default_config_matches_toml() {
    // Rust Default로 생성한 값과 default-config.toml을 파싱한 값이 동일해야 함.
    // 이 테스트가 실패하면 config.rs의 Default와 share/default-config.toml이 불일치.
    let from_rust = OxiosConfig::default();
    let from_toml_str = include_str!("../../../share/default-config.toml");
    let from_toml: OxiosConfig = toml::from_str(from_toml_str)
        .expect("default-config.toml이 유효하지 않습니다");

    assert_eq!(
        from_rust.kernel.max_agents, from_toml.kernel.max_agents,
        "kernel.max_agents 불일치: Rust={}, TOML={}",
        from_rust.kernel.max_agents, from_toml.kernel.max_agents
    );
    assert_eq!(
        from_rust.gateway.host, from_toml.gateway.host,
        "gateway.host 불일치: Rust={}, TOML={}",
        from_rust.gateway.host, from_toml.gateway.host
    );
    assert_eq!(
        from_rust.gateway.port, from_toml.gateway.port,
        "gateway.port 불일치: Rust={}, TOML={}",
        from_rust.gateway.port, from_toml.gateway.port
    );
    // 전체 필드 비교
    let rust_val = serde_json::to_value(&from_rust).unwrap();
    let toml_val = serde_json::to_value(&from_toml).unwrap();
    assert_eq!(
        rust_val, toml_val,
        "Rust Default와 default-config.toml 간 불일치 발견"
    );
}
```

### 2.5 Consolidation 프리셋 — 기존 타입에 통합

별도의 `ResolvedConsolidation` 타입을 도입하지 않고, 기존 `ConsolidationConfig`에 `preset` 필드와 `apply_preset()` 메서드만 추가:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationConfig {
    /// 프리셋: "conservative" | "balanced" | "aggressive" | "custom"
    /// "custom"이 아니면 프리셋 값이 나머지 필드를 덮어씀.
    /// 기본값: "balanced"
    #[serde(default = "default_preset")]
    pub preset: String,

    // ── 기존 29개 필드 (변경 없음) ──────────────────────────
    #[serde(default = "default_true")]
    pub dream_enabled: bool,
    // ... 나머지 필드 그대로 유지 ...
}

fn default_preset() -> String {
    "balanced".into()
}

impl ConsolidationConfig {
    /// 프리셋을 해석하여 self 필드에 반영.
    /// "custom"이면 아무것도 하지 않음 (개별 필드 사용).
    /// 커널 초기화 시점에 한 번 호출.
    pub fn apply_preset(&mut self) {
        let preset = match self.preset.as_str() {
            "conservative" => Self::conservative(),
            "aggressive" => Self::aggressive(),
            "custom" => return, // 개별 필드 그대로 사용
            _ => Self::balanced(), // 알 수 없는 값은 balanced로 폴백
        };
        *self = preset;
    }

    fn balanced() -> Self {
        Self::default() // 현재 기본값이 balanced
    }

    fn conservative() -> Self {
        Self {
            preset: "conservative".into(),
            dream_enabled: true,
            dream_interval_hours: 48,        // ← 2일 (기본 24시간)
            dream_min_sessions: 10,          // ← 더 많은 세션 누적 후 Dream
            hot_max_entries: 100,            // ← 2x
            warm_max_entries: 1000,          // ← 2x
            cold_max_entries: 50_000,        // ← 5x
            hot_token_budget: 5_000,         // ← 더 큰 컨텍스트
            decay_multiplier: 0.8,           // ← 느린 감쇠
            retention_days: 365,             // ← 1년 보존
            protection_demotion_stale_days: 90, // ← 3개월 후에만 강등
            ..Self::default()
        }
    }

    fn aggressive() -> Self {
        Self {
            preset: "aggressive".into(),
            dream_enabled: true,
            dream_interval_hours: 4,         // ← 4시간마다 Dream
            dream_min_sessions: 2,           // ← 적은 세션으로도 Dream
            hot_max_entries: 20,
            warm_max_entries: 100,
            cold_max_entries: 1_000,
            hot_token_budget: 2_000,
            decay_multiplier: 1.0,
            decay_threshold: 0.1,            // ← 더 높은 임계값 = 더 적극적 삭제
            retention_days: 30,              // ← 30일 보존
            protection_demotion_stale_days: 14,
            ..Self::default()
        }
    }
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            preset: default_preset(),
            dream_enabled: true,
            dream_interval_hours: 24,
            // ... 기존 기본값 그대로 ...
        }
    }
}
```

**설계 이유:**

- `ResolvedConsolidation` 별도 타입을 만들면 29개 필드를 두 구조체에 복제해야 함.
- `apply_preset(&mut self)` 방식은 기존 `ConsolidationConfig`를 그대로 사용하는 모든 코드가 변경 없이 작동.
- 커널 초기화 시 `config.memory.consolidation.apply_preset()` 한 번만 호출.

### 2.6 명령어 정리

```rust
// 변경 전:
//   oxios pkg install <name>     → "not yet implemented"
//   oxios marketplace install <name> → 작동

// 변경 후:
//   oxios marketplace install <name>  → 유지 (기존 동작)
//   oxios pkg install <name>          → marketplace install로 리다이렉트
//   oxios pkg list                    → skill list로 리다이렉트
```

`PkgAction::Install`을 marketplace로 위임:

```rust
async fn cmd_pkg(kernel: &Kernel, action: &PkgAction) -> Result<()> {
    match action {
        PkgAction::Install { source, branch } => {
            // marketplace install로 위임
            let api = kernel.handle().marketplace_api.clone();
            // ... 기존 marketplace 설치 로직 재사용 ...
        }
        PkgAction::List => {
            // 설치된 스킬 목록 (기존 marketplace list와 동일)
        }
        // ...
    }
}
```

### 2.7 AGENTS.md 수정

| 항목 | 변경 |
|------|------|
| 포트 3000 → 4200 | 88, 317, 324행 수정 |
| `max_agents` 기본값 10 명시 | AGENTS.md 파일 위치 테이블에 추가 불필요 (이미 TOML 기준) |

### 2.8 WORKSPACE_SUBDIRS 단일 정의

`WORKSPACE_SUBDIRS`는 워크스페이스 레이아웃에 대한 것이므로 `config.rs`가 아닌
**`onboarding.rs`에 단일 정의**를 두고 `main.rs`에서 import:

```rust
// onboarding.rs (기존에도 사용 중이므로 자연스러운 소스)
pub const WORKSPACE_SUBDIRS: &[&str] = &[
    "workspace",
    "workspace/memory",
    "workspace/memory/knowledge",
    "workspace/seeds",
    "workspace/sessions",
    "workspace/skills",
];

// main.rs
use crate::onboarding::WORKSPACE_SUBDIRS;
```

---

## 3. 마이그레이션 계획

### Phase 0: 코멘트 보존 (0.5일) ← 최우선

| 작업 | 파일 | 비고 |
|------|------|------|
| `toml_edit` 의존성 추가 | `Cargo.toml` | 바이너리 crate에만 |
| `cmd_config_set`을 `toml_edit` 기반으로 교체 | `main.rs` | 코멘트/포맷팅 보존 |
| 기존 `set_config_value` match arm 제거 | `main.rs` | `set_toml_dot`으로 대체 |
| 기존 `get_config_value`를 serde_json 기반으로 교체 | `main.rs` | 전체 필드 조회 가능 |
| 타입 안전 검증 (`get_existing_type` + `parse_toml_value`) | `main.rs` | 기존 필드 타입 존중 |

**Phase 0 완료 후 즉시 배포 가능.** 이것만으로도 사용자 경험이 근본적으로 개선됨.

### Phase 1: 기본값 통일 + 회귀 방지 (0.5일)

| 작업 | 파일 | 비고 |
|------|------|------|
| `max_agents` 기본값 10으로 통일 | `config.rs:762` | `default_max_agents()` |
| `test_default_config_matches_toml` 추가 | `config.rs` | CI에서 불일치 감지 |
| AGENTS.md 포트 3000→4200 | `AGENTS.md` | 3곳 |
| `WORKSPACE_SUBDIRS` 단일 정의 | `onboarding.rs` + `main.rs` | 중복 제거 |

### Phase 2: 추가 명령어 (0.5일)

| 작업 | 파일 | 비고 |
|------|------|------|
| `ConfigAction::List` 추가 | `main.rs` | `collect_leaf_keys` |
| `ConfigAction::Reset` 추가 | `main.rs` | 기본값으로 복원 |
| 에러 메시지에 `config list` 안내 | `main.rs` | 알 수 없는 키 오류 시 |

### Phase 3: Consolidation 프리셋 (0.5일)

| 작업 | 파일 | 비고 |
|------|------|------|
| `preset` 필드 + `apply_preset()` 추가 | `config.rs` | 기존 타입에 통합 |
| conservative / aggressive 프리셋 값 정의 | `config.rs` | balanced = 기존 기본값 |
| 커널 초기화 시 `apply_preset()` 호출 | `kernel.rs` | 한 번 |
| `default-config.toml`에 `preset = "balanced"` 추가 | `share/default-config.toml` | |
| 프리셋 설명 주석 추가 | `share/default-config.toml` | |

### Phase 4: 명령어 정리 (0.5일)

| 작업 | 파일 | 비고 |
|------|------|------|
| `PkgAction::Install`을 marketplace에 위임 | `main.rs` | 하위 호환 유지 |
| 도움말 텍스트 정리 | `main.rs` | `pkg`를 marketplace의 별칭으로 명시 |

---

## 4. 영향 범위

| 파일 | 변경 유형 | Phase |
|------|----------|-------|
| `Cargo.toml` (바이너리) | `toml_edit` 추가 | 0 |
| `src/main.rs` | config set/get 재구현, 명령어 추가/정리 | 0, 2, 4 |
| `crates/oxios-kernel/src/config.rs` | 기본값 통일, 프리셋, 회귀 테스트 | 1, 3 |
| `crates/oxios-kernel/src/onboarding.rs` | `WORKSPACE_SUBDIRS` pub 공개 | 1 |
| `share/default-config.toml` | 프리셋 필드 추가, 주석 | 3 |
| `AGENTS.md` | 포트 3000→4200 | 1 |
| `src/kernel.rs` | `apply_preset()` 호출 | 3 |

---

## 5. 성공 기준

### Phase 0 (필수)
- [ ] `oxios config set gateway.port 8080` 실행 후 config 파일의 **코멘트가 보존**됨
- [ ] `oxios config get memory.sqlite.embedding_dim` → `256` (기존 9개 외 필드)
- [ ] `oxios config set gateway.host "true"` → 호스트 필드에 문자열 "true" 저장 (boolean 아님)

### Phase 1 (필수)
- [ ] `test_default_config_matches_toml` 테스트 통과
- [ ] `max_agents` 기본값이 Rust/TOML 양쪽 모두 10
- [ ] AGENTS.md의 모든 포트 참조가 4200

### Phase 2 (권장)
- [ ] `oxios config list` → 전체 설정 키 출력
- [ ] `oxios config list memory` → memory.* 필드만 출력
- [ ] `oxios config reset gateway.port` → 기본값(4200)으로 복원

### Phase 3 (권장)
- [ ] `oxios config set memory.consolidation.preset aggressive` 작동
- [ ] 커널 시작 시 aggressive 프리셋이 29개 필드에 반영됨
- [ ] `oxios config set memory.consolidation.preset custom` + 개별 필드 설정 → custom 값 사용

### Phase 4 (권장)
- [ ] `oxios pkg install <name>` → marketplace install로 위임
- [ ] 기존 `oxios marketplace install` 하위 호환 유지
