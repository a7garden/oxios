# RFC-018: 설정 UX 개선

> **상태:** 📝 설계
> **날짜:** 2026-05-26
> **우선순위:** P2
> **범위:** `src/main.rs`, `crates/oxios-kernel/src/config.rs`, `share/default-config.toml`, `docs/AGENTS.md`
> **선행:** 없음
> **후행:** 없음

---

## 1. 동기

### 현재 문제

| # | 문제 | 심각도 |
|---|------|--------|
| 1 | `oxios config set`이 120개 필드 중 9개만 지원 | 🟡 |
| 2 | `max_agents` 기본값 불일치: TOML=10, Rust=16 | 🟡 |
| 3 | `gateway.host` 불일치: TOML=`0.0.0.0`, Rust=`127.0.0.1` | 🟡 |
| 4 | AGENTS.md에 포트 3000, 실제는 4200 | 🟢 |
| 5 | `memory.consolidation` 18개 필드에 프리셋 없음 | 🟡 |
| 6 | `oxios pkg install` vs `oxios marketplace install` 혼란 | 🟢 |
| 7 | `engine.default_model` 기본값 빈 문자열 — 에러 메시지 불친절 | 🟡 |
| 8 | config 수정 전 백업 없음 | 🟢 |
| 9 | 두 개의 동일한 `WORKSPACE_SUBDIRS` 배열 | 🟢 |

---

## 2. 설계

### 2.1 Dot-notation Config Set/Get

현재 9개 하드코딩 키를 동적 dot-notation 접근으로 교체:

```rust
// 변경 전: src/main.rs
fn get_config_value(config: &OxiosConfig, key: &str) -> Option<String> {
    match key {
        "engine.default_model" => Some(config.engine.default_model.clone()),
        "gateway.host" => Some(config.gateway.host.clone()),
        "gateway.port" => Some(config.gateway.port.to_string()),
        // ... 9개만
        _ => None,
    }
}

// 변경 후: serde_json 기반 동적 접근
fn get_config_value(config: &OxiosConfig, key: &str) -> Result<String> {
    let json = serde_json::to_value(config)
        .context("설정을 JSON으로 변환 실패")?;

    let value = json.pointer(&format!("/{}", key.replace('.', "/")))
        .ok_or_else(|| anyhow!("알 수 없는 설정 키: '{}'", key))?;

    // 값이 객체가 아닌 경우만 반환 (leaf nodes)
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

fn set_config_value(config: &mut OxiosConfig, key: &str, value: &str) -> Result<()> {
    // TOML 파싱으로 원본 보존 + 특정 키만 수정
    let config_path = get_config_path()?;
    let toml_str = std::fs::read_to_string(&config_path)?;

    // 백업 생성
    std::fs::write(
        config_path.with_extension("toml.bak"),
        &toml_str,
    )?;

    // TOML 파싱 → 수정 → 재작성
    let mut doc = toml_str.parse::<toml_edit::DocumentMut>()?;
    set_toml_value(&mut doc, key, value)?;
    std::fs::write(&config_path, doc.to_string())?;

    info!("설정 변경: {} = {}", key, value);
    Ok(())
}

fn set_toml_value(doc: &mut DocumentMut, key: &str, value: &str) -> Result<()> {
    let parts: Vec<&str> = key.split('.').collect();
    let mut table = doc.as_table_mut();

    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // leaf: 값 설정
            let parsed = parse_toml_value(value);
            table[*part] = parsed;
        } else {
            // 중간: 테이블 진입
            table = table.entry(*part)
                .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()))
                .as_table_mut()
                .ok_or_else(|| anyhow!("'{}'는 테이블이 아닙니다", parts[..=i].join(".")))?;
        }
    }
    Ok(())
}

fn parse_toml_value(value: &str) -> toml_edit::Item {
    if value == "true" { return toml_edit::value(true); }
    if value == "false" { return toml_edit::value(false); }
    if let Ok(n) = value.parse::<i64>() { return toml_edit::value(n); }
    if let Ok(n) = value.parse::<f64>() { return toml_edit::value(n); }
    toml_edit::value(value)
}
```

**의존성 추가:** `toml_edit = "0.22"` (TOML 포맷팅/코멘트 보존)

### 2.2 기본값 통일

```rust
// config.rs — 단일 소스 오브 트루스

impl Default for OxiosConfig {
    fn default() -> Self {
        Self {
            kernel: KernelConfig {
                max_agents: 10,            // TOML과 통일
                ..Default::default()
            },
            gateway: GatewayConfig {
                host: "0.0.0.0".into(),    // TOML과 통일
                port: 4200,
            },
            // ...
        }
    }
}
```

**또는** (더 나은 방법): Rust 기본값을 제거하고 항상 `default-config.toml`에서 읽기:

```rust
impl OxiosConfig {
    /// 기본 설정을 default-config.toml에서 로드
    pub fn defaults() -> Result<Self> {
        let default_toml = include_str!("../../share/default-config.toml");
        toml::from_str(default_toml).context("기본 설정 파싱 실패")
    }
}
```

### 2.3 Consolidation 프리셋

```toml
# config.toml

[memory.consolidation]
# 프리셋: "conservative" | "balanced" | "aggressive" | "custom"
preset = "balanced"

# 프리셋을 "custom"으로 설정하면 개별 필드가 사용됨
# preset이 설정되면 아래 값은 무시됨

# dream_enabled = true
# dream_interval_secs = 3600
# hot_retention_days = 7
# warm_retention_days = 30
# cold_retention_days = 365
# ... 나머지 13개 필드
```

```rust
// config.rs
#[derive(Debug, Deserialize, Serialize)]
pub struct ConsolidationConfig {
    pub preset: ConsolidationPreset,
    // 개별 필드들... (preset이 Custom일 때만 사용)
}

#[derive(Debug, Deserialize, Serialize)]
pub enum ConsolidationPreset {
    Conservative,  // 느린 감쇠, 긴 보존
    Balanced,      // 기본값
    Aggressive,    // 빠른 감쇠, 적극적 압축
    Custom,        // 개별 필드 사용
}

impl ConsolidationConfig {
    pub fn resolved(&self) -> ResolvedConsolidation {
        match &self.preset {
            ConsolidationPreset::Conservative => ResolvedConsolidation {
                dream_enabled: true,
                dream_interval_secs: 7200,
                hot_retention_days: 14,
                warm_retention_days: 60,
                cold_retention_days: 730,
                decay_rate: 0.01,
                compaction_enabled: true,
                ..Default::default()
            },
            ConsolidationPreset::Balanced => ResolvedConsolidation::default(),
            ConsolidationPreset::Aggressive => ResolvedConsolidation {
                dream_interval_secs: 1800,
                hot_retention_days: 3,
                warm_retention_days: 14,
                cold_retention_days: 90,
                decay_rate: 0.05,
                ..Default::default()
            },
            ConsolidationPreset::Custom => ResolvedConsolidation {
                dream_enabled: self.dream_enabled,
                dream_interval_secs: self.dream_interval_secs,
                // ... 개별 필드 매핑
            },
        }
    }
}
```

### 2.4 명령어 혼란 정리

```rust
// main.rs — pkg 서브명령어 정리

// 변경 전:
// oxios pkg install <name>   → "not yet implemented"
// oxios marketplace install <name> → 작동

// 변경 후: marketplace를 pkg의 alias로 통일
// oxios install <name>       → ClawHub 설치
// oxios search <query>       → ClawHub 검색
// oxios skill list           → 설치된 스킬 목록
```

### 2.5 AGENTS.md 수정

```markdown
# 변경 사항
- 포트: 3000 → 4200 (실제 코드와 통일)
- `max_agents` 기본값: 10 (TOML과 통일)
- `gateway.host`: 0.0.0.0 (TOML 기본값 명시)
```

### 2.6 코드 중복 제거

```rust
// 변경 전: WORKSPACE_SUBDIRS가 main.rs와 onboarding.rs에 각각 정의

// 변경 후: config.rs에 단일 정의
pub const WORKSPACE_SUBDIRS: &[&str] = &[
    "sessions", "seeds", "skills", "knowledge", "logs", "backups",
];

// main.rs와 onboarding.rs에서 import
use crate::config::WORKSPACE_SUBDIRS;
```

---

## 3. 마이그레이션 계획

### Phase 1: 기본값 통일 + 문서 (0.5일)

| 작업 | 비고 |
|------|------|
| `max_agents` 기본값 10으로 통일 | `config.rs` |
| `gateway.host` 의도적 차이 주석 추가 | 또는 통일 |
| AGENTS.md 포트/기본값 수정 | `docs/AGENTS.md` |
| `WORKSPACE_SUBDIRS` 단일 정의 | `config.rs` |

### Phase 2: Config set/get 일반화 (1일)

| 작업 | 비고 |
|------|------|
| `toml_edit` 의존성 추가 | `Cargo.toml` |
| dot-notation `get/set` 구현 | `main.rs` |
| config 수정 전 백업 | `.bak` 파일 |
| 기존 9개 하드코딩 제거 | `main.rs` |

### Phase 3: Consolidation 프리셋 (0.5일)

| 작업 | 비고 |
|------|------|
| `ConsolidationPreset` enum 정의 | `config.rs` |
| 프리셋 해석 로직 | `config.rs` |
| `default-config.toml` 업데이트 | `preset = "balanced"` |

### Phase 4: 명령어 정리 (0.5일)

| 작업 | 비고 |
|------|------|
| `oxios install` 통일 | marketplace → install alias |
| `oxios pkg` 제거 또는 redirect | 하위 호환 |

---

## 4. 영향 범위

| 파일 | 변경 |
|------|------|
| `config.rs` | 기본값 통일, 프리셋, WORKSPACE_SUBDIRS |
| `main.rs` | config set/get 일반화, 명령어 정리 |
| `onboarding.rs` | WORKSPACE_SUBDIRS import로 교체 |
| `default-config.toml` | 프리셋 추가, 주석 |
| `AGENTS.md` | 포트/기본값 수정 |
| `Cargo.toml` | `toml_edit` 추가 |

---

## 5. 성공 기준

- [ ] `oxios config set memory.consolidation.preset aggressive` 작동
- [ ] `oxios config get engine.default_model` → 현재 값 출력
- [ ] 존재하지 않는 키에 대해 명확한 에러 메시지
- [ ] config 수정 전 `.bak` 파일 자동 생성
- [ ] `max_agents`, `gateway.host` 기본값이 TOML과 일치
- [ ] AGENTS.md의 기술 정보가 코드와 일치
- [ ] `oxios install <skill>` 이 작동 (marketplace alias)
