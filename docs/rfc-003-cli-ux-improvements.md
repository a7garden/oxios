# RFC: CLI & Onboarding UX 개선 설계

> **상태:** 제안
> **영향 모듈:** `src/main.rs`, `crates/oxios-kernel/src/onboarding.rs`, `crates/oxios-kernel/Cargo.toml`, `Cargo.toml`
> **의존성 추가:** `console` (터미널 색상 + 스피너)

---

## 1. 현재 상태 분석

### 1.1 온보딩 (`onboarding.rs`)

| 항목 | 상태 | 비고 |
|------|------|------|
| 화살표 키 선택 | ✅ 완료 | `inquire` 기반 |
| Provider 선택 | ✅ 완료 | 모델 수 + 🔑 표시 |
| Model 선택 | ✅ 완료 | context window + reasoning 표시 |
| API 키 입력 | ⚠️ 개선 필요 | `CustomType` — 마스킹 안 됨 |
| auth.json 재사용 | ✅ 완료 | 자동 감지 + Confirm |
| 요약 + 확인 | ✅ 완료 | 박스 UI + Confirm |
| 온보딩 후 플로우 | ❌ 없음 | 완료 후 사용자가 직접 start 해야 함 |
| API 키 유효성 검증 | ❌ 없음 | 잘못된 키를 저장하고 나중에 실패 |

### 1.2 CLI 명령어 (`main.rs`)

| 항목 | 상태 | 비고 |
|------|------|------|
| `oxios start/serve` | ✅ 완료 | 명시적 데몬 시작 |
| `oxios onboard/setup` | ✅ 완료 | 명시적 온보딩 |
| `oxios reset` | ✅ 완료 | 확인 프롬프트 + `--yes` |
| `oxios doctor` | ✅ 완료 | 6개 항목 체크 |
| `oxios status` | ⚠️ 개선 필요 | 텍스트만, 색상 없음 |
| `oxios log` | ⚠️ 개선 필요 | `tail` 외부 명령 의존 |
| `oxios config show` | ⚠️ 개선 필요 | subcommand 필수 |
| 에러 메시지 | ⚠️ 개선 필요 | `anyhow` 에러가 그대로 노출 |
| 온보딩 게이트 메시지 | ❌ 없음 | 커널 조립 전 아무 피드백 없음 |
| Shell completion | ❌ 없음 | 탭 자동완성 불가 |

### 1.3 글로벌 문제

| 문제 | 설명 |
|------|------|
| **색상 없음** | 모든 출력이 흑백. `✓`, `✗`, `⚠` 기호는 있지만 색상이 없어 가독성 낮음 |
| **진행 표시 없음** | 커널 조립, MCP 초기화 등에 spinner 없음 |
| **reset이 PID를 하드코딩** | `oxios_home.join("oxios.pid")` — config의 `daemon.pid_file` 무시 |
| **첫 실행 가이드 부재** | `oxios`만 입력 → 온보딩 → 완료 → 끝. 다음에 뭘 해야 하는지 불명확 |
| **config subcommand 필수** | `oxios config`만 치면 에러. `oxios config show`가 기본값이어야 함 |
| **--yes가 global이 아님** | `reset --yes`만 있고 다른 파괴적 명령엔 없음 |

---

## 2. 개선 사항 (우선순위순)

### P0: 즉시 적용 — 실제 사용에 지장이 있는 것

#### 2.1 API 키 입력 시 마스킹

**문제:** `CustomType<String>`은 입력 시 텍스트가 그대로 보임. 터미널 히스토리에 남음.

**해결:** `inquire::Password` 사용.

```rust
fn prompt_api_key(provider: &str) -> anyhow::Result<String> {
    println!();
    println!("  [2/{}] Enter your {} API key:", TOTAL_STEPS, provider);

    inquire::Password::new("  API key:")
        .with_display_mode(inquire::password::PasswordDisplayMode::Masked)
        .with_placeholder("sk-...")
        .without_confirmation()
        .prompt()
        .map_err(Into::into)
}
```

**영향:** `onboarding.rs` 1개 함수

---

#### 2.2 `oxios config` 기본값 = `show`

**문제:** `oxios config`만 치면 `arg_required_else_help` 에러.

**해결:** `ConfigAction`에 기본값 설정. Clap의 `#[command(subcommand_required = false)]` + `Option<ConfigAction>`.

```rust
Config {
    #[command(subcommand)]
    action: Option<ConfigAction>,
},
```

디스패치에서 `None`이면 `ConfigAction::Show`로 처리.

**영향:** `main.rs` 타입 정의 + 디스패치

---

#### 2.3 reset의 PID 경로 하드코딩 수정

**문제:** `cmd_reset`이 `oxios_home.join("oxios.pid")`을 사용하지만, 실제 PID 파일 경로는 config에 의해 결정됨.

**해결:** `cmd_reset` 호출 전에 config를 읽어 PID 파일 경로를 전달.

```rust
Some(Command::Reset { yes }) => {
    let pid_file = oxios_kernel::config::expand_home(&config.daemon.pid_file);
    return cmd_reset(&oxios_home, *yes, &pid_file);
}
```

**영향:** `main.rs` fast-path + `cmd_reset` 시그니처

---

#### 2.4 온보딩 완료 후 "Start now?" 플로우

**문제:** 온보딩 완료 → 사용자가 직접 `oxios start`를 쳐야 함. UX 단절.

**해결:** `run_onboarding`이 `true`를 반환하면, `main()`에서 `Confirm("Start daemon now?")` 표시. 긍정하면 `cmd_serve` 또는 `daemon.start` 실행.

```rust
if needs_kernel && !has_credentials(&config) {
    let completed = run_onboarding(&oxios_home, &mut config)?;
    if completed {
        config = load_config(&config_path)?;
        let start_now = Confirm::new("  Start daemon now?")
            .with_default(true)
            .prompt()?;
        if !start_now {
            println!("  Run `oxios start` when ready.");
            return Ok(());
        }
        // fall through to kernel assembly + daemon start
    } else {
        return Ok(());
    }
}
```

**영향:** `main.rs` onboarding gate

---

### P1: UX 품질 — 있으면 좋고 없어도 되지만, 차이가 큼

#### 2.5 터미널 색상

**문제:** 모든 출력이 흑백.

**해결:** `console` 크레이트 사용. 핵심 지점에만 색상 적용.

```rust
use console::style;

// status / doctor / onboarding 공통
println!("  {} {}", style("✓").green(), style("Config file present").bold());
println!("  {} {}", style("✗").red(), style("Config file missing").bold());
println!("  {} {}", style("⚠").yellow(), style("Daemon not running").bold());
```

**적용 위치:**
- `cmd_status` — 모든 라인
- `cmd_doctor` — 모든 체크 항목
- `cmd_reset` — 경고 메시지
- `onboarding.rs` — 배너, 성공 메시지

**의존성:** `console = "0.15"` (inquire가 이미 crossterm을 끌어오므로 빌드 타임 영향 미미)

---

#### 2.6 커널 조립 스피너

**문제:** 커널 조립에 1-3초가 걸리는데 아무 피드백이 없음.

**해결:** `console::Term` + 간단한 스피너. (또는 `indicatif`의 `SpinnerStyle`)

```rust
let term = console::Term::stderr();
term.write_str("  ⠋ Starting Oxios...")?;
let kernel = Kernel::builder()...build().await?;
term.clear_line()?;
```

**영향:** `main.rs` 커널 조립 지점 1곳

---

#### 2.7 `oxios log` — tail 외부 의존 제거

**문제:** `tail` 명령이 시스템에 없으면 실패. Windows에서도 안 됨.

**해결:** Rust로 구현.

```rust
fn tail_file(path: &Path, lines: usize) -> Result<String> {
    let content = std::fs::read_to_string(path)?;
    let line_list: Vec<&str> = content.lines().rev().take(lines).collect();
    let mut result = line_list;
    result.reverse();
    Ok(result.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n"))
}
```

**영향:** `main.rs` log 핸들러 1곳

---

#### 2.8 `oxios doctor` — oxi CLI 존재 확인 추가

**현재 체크 6개 → 8개로 확장:**

| # | 체크 항목 | 기준 |
|---|----------|------|
| 1 | config 파일 | exists |
| 2 | credentials | resolve 성공 |
| 3 | workspace 디렉토리 | exists |
| 4 | daemon 상태 | Running |
| 5 | MCP 서버 | count > 0 |
| 6 | 기본 모델 | non-empty |
| 7 **(신규)** | oxi CLI 설치 | `which oxi` 또는 `~/.oxi/auth.json` 존재 |
| 8 **(신규)** | 포트 사용 가능 | gateway port가 이미 점유되었는지 |

**영향:** `cmd_doctor` 함수 확장

---

### P2: Nice-to-have — 장기적으로 갖추면 좋음

#### 2.9 Shell Completion

**문제:** 탭 자동완성 불가.

**해결:** `clap_complete`로 bash/zsh/fish 스크립트 생성.

```rust
// 새 서브커맨드
Completion { shell: clap_complete::Shell },
```

`oxios completion bash > ~/.local/share/bash-completion/completions/oxios`

**영향:** `Cargo.toml` + `main.rs` 1개 서브커맨드

---

#### 2.10 API 키 유효성 사전 검증

**문제:** 온보딩에서 틀린 API 키를 저장하고, 첫 실행에서 에러.

**해결:** 온보딩 마지막에 실제 API 호출 1건 (models list) 로 검증.

```rust
// onboarding.rs — persist_config 직전
if !api_key.is_empty() {
    let valid = validate_api_key(provider, api_key).await;
    if !valid {
        let retry = Confirm::new("  Key validation failed. Save anyway?")
            .with_default(false)
            .prompt()?;
        if !retry { return Ok(false); }
    }
}
```

**영향:** `onboarding.rs` + `credential.rs`에 `validate_api_key` 추가

---

#### 2.11 에러 메시지 한글화 + 가이드

**문제:** `anyhow` 에러가 그대로 노출. 예: "failed to list agents: ..."

**해결:** 메인 디스패치의 에러 핸들링 개선.

```rust
// main() 최하단에
if let Err(e) = run(cli).await {
    eprintln!("\n  {} {}", style("error:").red().bold(), e);
    eprintln!("  Run `oxios doctor` for diagnostics.\n");
    std::process::exit(1);
}
```

**영향:** `main.rs` 에러 핸들링

---

#### 2.12 `oxios models` — 사용 가능한 모델 목록

**문제:** 현재 모델 목록을 보려면 온보딩을 다시 실행해야 함.

**해결:** `oxios models` 서브커맨드 추가.

```rust
/// List available models for the configured provider.
Models {
    /// Show models for a specific provider (default: current provider).
    #[arg(short, long)]
    provider: Option<String>,
},
```

**영향:** `main.rs` 1개 서브커맨드

---

## 3. 변경 영향 요약

### 파일별 변경량 추정

| 파일 | P0 변경 | P1 변경 | P2 변경 |
|------|---------|---------|---------|
| `onboarding.rs` | API 키 마스킹 | 색상 | 키 검증 |
| `main.rs` | config 기본값, PID 수정, 온보딩 후 플로우 | 색상, 스피너, log 개선, doctor 확장 | completion, models, 에러 핸들링 |
| `Cargo.toml` | — | `console` 추가 | `clap_complete` 추가 |
| `crates/oxios-kernel/Cargo.toml` | — | `console` 추가 | — |

### 의존성 추가

| 크레이트 | 용도 | 트리에 미치는 영향 |
|----------|------|---------------------|
| `console 0.15` | 터미널 색상, 스피너 | `inquire`가 이미 `crossterm`을 끌어오므로 영향 미미 |
| `clap_complete 4` (P2) | Shell completion 스크립트 생성 | `clap`만 있으면 됨 |

### 위험도

| 변경 | 위험 | 이유 |
|------|------|------|
| API 키 마스킹 | 낮음 | `inquire` 기능 교체만 |
| config 기본값 | 낮음 | Clap 설정만 변경 |
| PID 경로 수정 | 낮음 | 매개변수 추가만 |
| 온보딩 후 start | 중간 | 커널 조립 타이밍과 겹침 |
| 색상 | 낮음 | 출력만 변경, 로직 변경 없음 |
| 스피너 | 중간 | 터미널 상태 관리 필요 |

---

## 4. 구현 순서 제안

```
Phase 1 (P0): 2.1 → 2.2 → 2.3 → 2.4    ← 바로 구현 가능, 즉각적 UX 개선
Phase 2 (P1): 2.5 → 2.6 → 2.7 → 2.8     ← console 추가 후 한 번에
Phase 3 (P2): 2.9 → 2.10 → 2.11 → 2.12  ← 시간 될 때
```

P0는 즉시 구현 가능하고 모두 1-2개 파일만 변경하는 작은 작업입니다.
P1은 `console` 크레이트 하나만 추가하면 됩니다.
P2는 각각 독립적이어서 선택적으로 구현 가능합니다.
