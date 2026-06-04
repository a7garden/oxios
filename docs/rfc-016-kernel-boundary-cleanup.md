# RFC-016: Kernel 경계 정리 — `oxios-memory` 추출 및 명칭 통일

> **날짜**: 2026-06-04
> **상태**: 초안
> **범위**: `crates/oxios-kernel` (분할) · `crates/oxios-memory` (신설) · `surface/oxios-web` (이름) · `src/` (디렉터리화) · `benches/` (이동)
> **관련 RFC**: RFC-008 (메모리 통합), RFC-012 (메모리 통합), RFC-014 (oxi-sdk 0.26 마이그레이션)
> **선행 진단**: 본 저장소 AGENTS.md의 "Kernel is intentionally monolithic" 원칙(§10)은 유지한다.

---

## 1. 배경/동기

### 1.1 코드 분포의 비대칭

워크스페이스의 Rust 코드 분포(`find … -name "*.rs" | xargs wc -l`, 2026-06-04 기준):

| 위치 | 파일 수 | LoC | 비율 |
|------|------|------|------|
| `crates/oxios-kernel` | 137 | 53,487 | 64.1% |
| `surface/oxios-web` | 26 | 10,400 | 12.4% |
| `crates/oxios-markdown` | 21 | 7,537 | 9.0% |
| `crates/oxios-ouroboros` | 14 | 3,139 | 3.7% |
| `src/` (binary) | 7 | 4,292 | 5.1% |
| `crates/oxios-mcp` | 3 | 1,458 | 1.7% |
| `crates/oxios-gateway` | 10 | 1,304 | 1.5% |
| `channels/*` | 10 | 1,730 | 1.9% |

`oxios-kernel`이 64%를 점유한다. AGENTS.md §10은 이를 "의도적 모놀리식 단일 크레이트"로 명시하지만, 그 *내부*에서 **`memory/` 가 13,360 LoC, 25%** 를 점유한다. memory/ + embedding(.rs + embedding/gguf/, ~654 LoC)을 합치면 **약 14,000 LoC — kernel의 4 분의 1**이 독립 도메인에 묶여 있다.

### 1.2 memory가 vertical slice인 이유

```
crates/oxios-kernel/src/memory/         29 files / 13,360 LoC
├── store.rs            MemoryManager, MemoryEntry, MemoryTier
├── decay.rs            DecayEngine (중요도 감쇠)
├── dream.rs            DreamProcess (수면 중 통합)
├── hnsw.rs             HnswIndex, HnswMemoryIndex (ANN 검색)
├── hyperbolic.rs       HyperbolicEmbedding, mobius_add
├── graph.rs            MemoryGraph
├── sqlite_store.rs     SqliteMemoryStore       [sqlite-memory]
├── database.rs         MemoryDatabase          [sqlite-memory]
├── cache.rs            cache                    [sqlite-memory]
├── search/             BM25 + RRF + vector      [sqlite-memory]
├── chunking.rs         TextChunk, chunk_fixed
├── auto_classify.rs    자체 LLM 호출 (분류)
├── auto_memory_bridge  Knowledge ↔ Memory 양방향 동기화
├── flash_attention.rs  FlashAttention (메모리 추정)
├── proactive.rs        ProactiveRecall
├── sona.rs             SONA 적응형
├── embedding_cache.rs  LRU 임베딩 캐시
├── embedding_viz.rs    2D 투영 (메모리 맵)
├── quota.rs ← (현재 budget.rs)
├── … 등
```

**memory는 다른 모듈을 import하지만, 다른 모듈은 memory를 import한다** — *방향이 단방향*이다. 즉 memory는 깔끔한 vertical slice이며, AGENTS.md가 경고하는 "순환 의존성"을 *유발하지 않는다*.

이는 kernel의 다른 어떤 서브시스템(예: tools, skill, persona)에도 해당하지 않는 성질이다. 따라서 memory는 추출 가치가 가장 큰 모듈이다.

### 1.3 자잘한 명칭/잔재 문제

지난 진단(2026-06-04, 이전 대화)에서 다음 5건이 식별됐다:

1. **`tools/kernel/` 디렉터리가 `oxios-kernel` crate 안에 중첩** — 경로 `oxios_kernel::tools::kernel::agent_tool`은 kernel-handler-style tool 모음이지만, 이름이 "kernel"이라 모듈 위계가 흐려진다.
2. **`memory/budget.rs` ↔ `budget.rs` 이름 충돌** — 전자는 memory quota, 후자는 orchestration cost. 같은 crate 안에 같은 명사가 두 책임을 가진다.
3. **`surface/oxios-web/src/channel.rs` ↔ `channels/`(top-level) 충돌** — web이 과거 channel의 하나로 시작해 surface로 확대됐기 때문. 디렉터리/파일 명명 충돌.
4. **`src/cmd_run.rs`, `src/cmd_update.rs` 평면 누워 있음** — 두 번째 명령어 추가 시 비대칭 누적.
5. **kernel `src/` 안에 `.md` 작업 잔재 3건** — `tools/retrieval-output.md`, `tools/kernel/impl-output.md`, `kernel_handle/impl-output.md`. AGENTS.md의 "no analysis files in project root" 원칙이 src/에는 적용되지 않은 결과.
6. **루트 `benches/` 와 `benchmarks/oxios-bench/` 공존** — 같은 단어("benchmark")로 두 메커니즘이 공존.

---

## 2. 목표 / 비목표

### 목표

1. **`oxios-memory` crate 신설** — memory/, embedding, embedding/gguf/를 단일 crate로 추출.
2. **KernelHandle에 `MemoryApi` 신설** — memory 관련 13개 API에 14번째 추가.
3. **명칭/잔재 6건 정리**.
4. **Default feature 단순화** — `sqlite-memory`를 oxios-kernel에서 oxios-memory로 이동.
5. **Browser 일원화 (Phase E)** — oxi-sdk의 `native_browser_tools()`로 browser 도구 *일원화*. kernel의 `BrowserTool`/`BrowserApi` *제거*. **AGENTS.md "oxi-sdk 재구현 금지" 위반 해소**.

### 비목표 (의식적 보류)

- `oxios-kernel`을 *그 외 어떤* 모듈로도 더 분할하지 않는다. supervisor/skill/tools/a2a 등은 일체 손대지 않는다.
- `oxios-ouroboros` 이름을 유지한다. 원본 Ouroboros 오픈소스(Q00) 프로젝트의 프로토콜을 그대로 이식한 것이며, 이름 변경은 attribution 손상.
- `oxios-mcp` 흡수하지 않는다. 1,458 LoC, 3 files로 크기는 작지만 흡수는 별도 결정이 필요.
- `oxios-kernel`을 "`oxios-kernel-core` + `oxios-kernel`"로 쪼개지 않는다. 이름이 분리가 아닌 분장(扮裝)이 된다.
- kernel이 큰 것 자체는 정상으로 간주한다 ("the core"는 큰 법이다).
- oxi-sdk의 browser 도구(browse/browse_extract/browse_session)를 *oxios의 커스텀 도구로 재구현*하지 않는다. 커스터마이즈가 필요하면 oxi-sdk에 PR을 올리는 것을 원칙으로 한다.

---

## 3. 설계

### 3.1 `oxios-memory` crate 신설

#### 3.1.1 이동 대상

```
crates/oxios-kernel/src/memory/         →  crates/oxios-memory/src/memory/  (예외: auto_memory_bridge.rs)
crates/oxios-kernel/src/embedding.rs    →  crates/oxios-memory/src/embedding.rs
crates/oxios-kernel/src/embedding/gguf/ →  crates/oxios-memory/src/embedding/gguf/
crates/oxios-kernel/src/memory/auto_memory_bridge.rs  →  crates/oxios-kernel/src/auto_memory_bridge.rs
```

**예외 1건:** `auto_memory_bridge.rs`는 memory 디렉터리 안에 있지만 본질은 *orchestration* 로직(knowledge ↔ memory 양방향 동기화)이다. §6.4에서 상세히 다루듯, 이 파일은 kernel에 *남긴다* (memory crate로 가지 않음). 28개 파일이 oxios-memory로 이동하고 1개는 kernel 루트로 *승격*된다.

총 **약 14,000 LoC, 31 files**가 oxios-memory로 이동한다. kernel은 53,487 → 39,500 LoC 정도로 줄어들어 *전체의 47%*가 된다. memory가 그래도 큰 것은 사실이지만, 이제 *독립 crate*로 분리되어 *명확한 책임*을 가진다.

#### 3.1.2 디렉터리 구조 (목표)

```
crates/oxios-memory/
├── Cargo.toml
└── src/
    ├── lib.rs                          re-exports
    ├── embedding.rs                    EmbeddingProvider, EmbeddingVector, TfIdfEmbeddingProvider
    │                                   (kernel의 embedding.rs 패턴 그대로)
    ├── embedding/                      ← 같은 이름 file + dir 콤보 (Rust 관용)
    │   └── gguf/                       GgufEmbeddingProvider, GgufModelLoader   [gguf]
    │       └── mod.rs
    └── memory/
        ├── mod.rs                      MemoryManager, MemoryEntry, MemoryTier, MemoryType
        ├── store.rs                    MemoryManager 구현
        ├── decay.rs                    DecayEngine
        ├── dream.rs                    DreamProcess, DreamCheckpoint, DreamReport
        ├── hnsw.rs                     HnswIndex, HnswMemoryIndex
        ├── hyperbolic.rs               HyperbolicEmbedding
        ├── graph.rs                    MemoryGraph
        ├── chunking.rs                 TextChunk, chunk_fixed, chunk_paragraphs
        ├── compaction.rs               CompactionTree
        ├── auto_classify.rs            자체 LLM 기반 분류
        ├── auto_protect.rs             자동 보호 정책
        ├── flash_attention.rs          FlashAttention
        ├── normalizer.rs               텍스트 정규화
        ├── proactive.rs                ProactiveRecall
        ├── root_index.rs               RootIndex, RootEntry, TopicEntry
        ├── sona.rs                     SONA 적응형 메모리
        ├── embedding_cache.rs          LRU 임베딩 캐시
        ├── embedding_viz.rs            2D 투영
        ├── migrate.rs                  스키마 마이그레이션 도구
        ├── migration.rs                [sqlite] MigrationReport
        ├── quota.rs                    ← (이전 memory/budget.rs, 명칭 변경)
        ├── database.rs                 [sqlite] MemoryDatabase
        ├── sqlite_store.rs             [sqlite] SqliteMemoryStore
        ├── cache.rs                    [sqlite] cache
        └── search/                     [sqlite] BM25 + RRF + vector
            ├── mod.rs
            ├── bm25.rs
            ├── rrf.rs
            └── vector.rs
```

#### 3.1.3 `oxios-memory/Cargo.toml` (개요)

```toml
[package]
name = "oxios-memory"
version = "1.0.2"
edition = "2021"
description = "Tiered agent memory — decay, dream, HNSW, hyperbolic, sqlite, GGUF"
license = "MIT"

[dependencies]
oxi-sdk        = { workspace = true }
tokio          = { workspace = true }
serde          = { workspace = true }
serde_json     = { workspace = true }
anyhow         = { workspace = true }
thiserror      = { workspace = true }
chrono         = { workspace = true }
parking_lot    = { workspace = true }
uuid           = { workspace = true }
tracing        = { workspace = true }
regex          = { workspace = true }
sha2           = { workspace = true }
once_cell      = { workspace = true }
rand           = "0.8"

[target.'cfg(target_arch = "aarch64")'.dependencies]
oxibrowser-core = { workspace = true }   # GGUF 로더가 aarch64에서 native 실행

[features]
default = ["sqlite"]       # 일반적인 사용 시 sqlite backend 채택
sqlite = []                # SQLite 기반 영속 메모리 (database, cache, search)
gguf   = []                # GGUF 임베딩 (aarch64 native, 다른 arch는 외부 API)
```

#### 3.1.4 의존성 그래프 (변경 후)

```
                        ┌─────────────┐
                        │  oxi-sdk    │  (단일 외부)
                        └──────┬──────┘
                               │
        ┌──────────────┬───────┴───────┬──────────────┐
        │              │               │              │
   ┌────▼─────┐  ┌─────▼─────┐  ┌──────▼──────┐  ┌────▼────┐
   │ ouroboros│  │  memory   │  │  markdown   │  │   mcp   │
   └────┬─────┘  └─────┬─────┘  └──────┬──────┘  └────┬────┘
        │              │               │              │
        │              │               │              │
        └──────────────┴───────┬───────┴──────────────┘
                              │
                        ┌─────▼─────┐
                        │   kernel  │   (이전 53k → 39k LoC)
                        └─────┬─────┘
                              │
                        ┌─────▼─────┐
                        │  gateway  │
                        └─────┬─────┘
                              │
              ┌───────────────┼───────────────┐
              │               │               │
        ┌─────▼─────┐  ┌──────▼──────┐  ┌─────▼─────┐
        │  binary   │  │ surface/web │  │  channels │
        └───────────┘  └─────────────┘  └───────────┘
```

핵심 변화: **`oxios-memory`가 `oxios-kernel`과 `oxi-sdk` 모두의 의존 대상**으로 들어왔다. kernel은 더 이상 memory 코드를 직접 포함하지 않고, `Arc<oxios_memory::MemoryManager>` 형태로 *참조*만 한다.

### 3.2 KernelHandle `MemoryApi` 신설

현재 memory 조작은 `kernel_handle/agent_api.rs` 안에 분산돼 있다 (`AgentApi`의 `memory_manager: Arc<MemoryManager>` 필드). 14번째 API로 분리한다.

```
crates/oxios-kernel/src/kernel_handle/
├── mod.rs
├── agent_api.rs        ← memory_manager 필드 제거
├── memory_api.rs       ← NEW: MemoryApi
├── …
```

```rust
// memory_api.rs (개요)
pub struct MemoryApi {
    pub(crate) memory_manager: Arc<MemoryManager>,
    pub(crate) hnsw_index: Option<Arc<HnswMemoryIndex>>,
}

impl MemoryApi {
    pub async fn recall(&self, agent_id: &AgentId, query: &str) -> Result<Vec<SemanticHit>>;
    pub async fn store(&self, agent_id: &AgentId, entry: MemoryEntry) -> Result<()>;
    pub async fn decay_status(&self) -> DecayReport;
    pub async fn run_dream(&self) -> Result<DreamReport>;
    pub fn quota(&self) -> MemoryQuota;
    // …
}
```

`AgentApi`의 `memory_manager` 필드는 제거하고, 대신 `memory: MemoryApi` (또는 `Arc<MemoryApi>`) 필드를 가진다. `KernelHandle::memory()` accessor 신설.

**하위 호환:** 기존 `use oxios_kernel::MemoryManager`는 `pub use oxios_memory::MemoryManager;` re-export로 그대로 동작. 단, `kernel_handle::AgentApi`의 `memory_manager` *필드 직접 접근*은 `pub(crate)`이므로 깨지지 않음 (외부 consumer는 accessor만 사용했었음).

### 3.3 명칭/잔재 정리

#### 3.3.1 `tools/kernel/` → `tools/builtin/`

```
crates/oxios-kernel/src/tools/kernel/   →   crates/oxios-kernel/src/tools/builtin/
```

이유: "kernel"이라는 단어가 *crate 이름*과 *디렉터리 이름*으로 중첩되면 grep/menu 자동완성/문서 link에서 모호하다. "builtin"은 "kernel이 자체 제공하는 tool 묶음"이라는 정확한 의미를 전달한다 (`browser`, `mcp`, `exec` tool과 대비).

영향: `tools/mod.rs`의 `pub mod kernel;` → `pub mod builtin;`. 9개 파일의 `mod.rs`만 수정하면 모듈 경로 전체가 자동으로 따라옴. 또한 `tools/kernel/impl-output.md` 잔재도 함께 삭제.

#### 3.3.2 `memory/budget.rs` → `memory/quota.rs`

```
crates/oxios-kernel/src/memory/budget.rs  →  crates/oxios-memory/src/memory/quota.rs
```

이유: kernel *루트*의 `budget.rs`는 orchestration cost (LLM 토큰 비용/시간/메모리 한도). memory의 `budget.rs`는 *memory curation quota* (per-type entry 수 제한). 같은 단어가 두 책임을 가리킨다.

- `quota`는 memory 한도(상한)를 명확히 가리키는 단어.
- `MemoryBudget`, `CurationCandidate` 타입은 그대로 두고 *파일명만* 변경 (외부 사용에 영향 없음).
- kernel 루트 `budget.rs`(BudgetManager)는 변경 없음.

#### 3.3.3 `surface/oxios-web/src/channel.rs` → `web_channel.rs`

```
surface/oxios-web/src/channel.rs        →  surface/oxios-web/src/web_channel.rs
```

이유: `channels/`(top-level, *메시지 채널* — CLI/Telegram/Web)과 `surface/oxios-web/src/channel.rs`(web이 channel이었을 때의 잔재)는 *다른 개념*을 가리키지만 grep이 섞어 보여준다.

- web이 *surface*로 진화한 것은 의도된 발전이므로 코드/이름 모두 surface 컨텍스트로 정렬.
- 변경: `lib.rs`의 `pub mod channel;` → `pub mod web_channel;`, 그리고 `plugin.rs`, `routes/*.rs`에서 `use crate::channel` → `use crate::web_channel` (rg로 일괄 치환).

#### 3.3.4 `src/cmd_*.rs` → `src/commands/`

```
src/cmd_run.rs     →  src/commands/run.rs
src/cmd_update.rs  →  src/commands/update.rs
(신규)             →  src/commands/mod.rs
```

```
src/
├── main.rs          (mod commands;)
├── commands/
│   ├── mod.rs       (pub mod run; pub mod update;)
│   ├── run.rs
│   └── update.rs
├── kernel.rs        (변경 없음)
├── otel.rs
├── surface.rs
└── web_dist.rs
```

이유: 두 번째 명령어(현재 cmd_update) 추가 시 비대칭이 누적되는 패턴을 차단. 세 번째 명령어부터는 `commands/foo.rs`만 만들면 끝.

#### 3.3.5 `*.md` 작업 잔재 삭제

```
crates/oxios-kernel/src/tools/retrieval-output.md       ✗ 삭제
crates/oxios-kernel/src/tools/kernel/impl-output.md      ✗ 삭제 (3.3.1과 함께)
crates/oxios-kernel/src/kernel_handle/impl-output.md     ✗ 삭제
```

AGENTS.md는 "no analysis files in project root"라 못 박지만 src/에 *더욱 엄격하게* 적용돼야 한다 (빌드/문서 어느 쪽에도 기여하지 않음).

#### 3.3.6 `benches/` vs `benchmarks/` 해소

현재:
- `/Volumes/MERCURY/PROJECTS/oxios/benches/kernel_bench.rs` — Cargo conventional top-level bench, root `Cargo.toml`의 `[[bench]]`로 등록.
- `/Volumes/MERCURY/PROJECTS/oxios/benchmarks/oxios-bench/` — workspace 멤버로 등록된 별도 통합 평가 스위트.

`benches/kernel_bench.rs`는 *kernel crate 내부의 마이크로 벤치*인데 root에 떠 있어 crate와 떨어져 있다. Cargo 관용은 crate 내 `benches/` 디렉터리다.

```
/Volumes/MERCURY/PROJECTS/oxios/benches/kernel_bench.rs        →  crates/oxios-kernel/benches/state_store.rs
/Volumes/MERCURY/PROJECTS/oxios/benches/                      →  (디렉터리 삭제)
/Volumes/MERCURY/PROJECTS/oxios/Cargo.toml                    →  [[bench]] 섹션 삭제
```

`benchmarks/oxios-bench/`는 *워크스페이스 레벨 통합 시나리오 평가*의 별도 공간이므로 유지하되, AGENTS.md / README에 **"kernel 마이크로벤치는 crate 내 `benches/`, 통합 평가는 `benchmarks/`"** 로 명시한다.

### 3.4 Default feature 단순화

```toml
# crates/oxios-kernel/Cargo.toml (변경)
[features]
default = []                       # 이전: ["browser", "sqlite-memory", "embedding-gguf"]
browser = ["oxi-sdk/native-browser"]

# crates/oxios-memory/Cargo.toml (신설)
[features]
default = ["sqlite"]               # 일반 사용 시 SQLite backend 채택
sqlite = []
gguf   = []

# 루트 Cargo.toml (변경)
[features]
default = ["web", "cli", "browser"]   # 이전: ["web", "cli", "browser", "sqlite-memory"]
web       = ["dep:oxios-web"]
cli       = ["dep:oxios-cli"]
telegram  = ["dep:oxios-telegram"]
browser   = ["oxios-kernel/browser"]
otel      = ["oxios-kernel/otel", "dep:opentelemetry", "dep:opentelemetry_sdk"]
```

변경 의도:
- `--no-default-features --features cli` 같은 *headless* 빌드가 깔끔히 떨어진다 (이전엔 sqlite-memory까지 끌려옴).
- 사용자가 `--features oxios-memory/gguf` 같은 *per-crate* 기능 게이트를 명시할 수 있다.
- binary(`oxios`)의 default는 web+cli+browser로 줄여 임베디드/서버 빌드를 명확히 한다.

### 3.5 Browser 일원화 (Phase E)

#### 3.5.1 결정

> **oxi-sdk의 `native_browser_tools()`로 browser 도구를 *일원화*한다.** `oxios-kernel`의 `BrowserTool`/`BrowserApi`는 *완전히 제거*한다. 이로써 AGENTS.md의 "oxi-sdk 재구현 금지" 위반을 해소한다.

#### 3.5.2 제거 대상

```
crates/oxios-kernel/src/tools/browser/                          (전체 디렉터리)
    mod.rs
    browser_tool.rs

crates/oxios-kernel/src/kernel_handle/browser_api.rs           (단일 파일)

crates/oxios-kernel/src/lib.rs                                 (BrowserTool, BrowserApi re-export 제거)
crates/oxios-kernel/src/tools/mod.rs                            (pub mod browser; 제거)
crates/oxios-kernel/src/tools/kernel_bridge.rs                   ("browser" in tool_names; browser 등록 코드 제거)
crates/oxios-kernel/src/tools/registration.rs                   (browser 등록 로직 제거)
crates/oxios-kernel/Cargo.toml                                 (oxibrowser-core direct dep 제거; browser feature 제거)

src/kernel.rs                                                   (build_browser_api() / build_browser_api_value() 제거)
```

**`oxibrowser-core` direct 의존도 제거.** oxi-sdk가 transitive하게 가져오므로, oxios가 *직접* 의존할 이유가 없다.

#### 3.5.3 추가 대상 (binary에서 oxi-sdk 호출)

```
src/kernel.rs:
    fn build_browser_tool_registry(config: &OxiosConfig) -> Option<Arc<ToolRegistry>> {
        if !config.browser.enabled {
            return None;
        }
        // oxi-sdk의 BrowseConfig와 OxiosConfig::BrowserConfig를 매핑
        let browse_config = map_browser_config(&config.browser);
        Some(oxi_sdk::native_browser_tools_with_config(browse_config).await?)
    }

    // KernelBuilder가 ToolRegistry를 받아 agent builder에 전달
    let agent_builder = oxi_sdk::AgentBuilder::new()
        .kernel_tools(bridge)
        .with_optional_browser(build_browser_tool_registry(&config));
```

핵심: **binary가 `KernelBuilder` 단계가 아니라 `AgentBuilder` 단계에서 browser를 등록**한다. kernel은 browser state를 *모름*.

#### 3.5.4 config 매핑

`OxiosConfig::BrowserConfig`는 *유지* (사용자 노출 config 보존). binary의 `map_browser_config()`가 우리 `BrowserConfig` → oxi-sdk의 `BrowseConfig`로 변환한다. 사용자 facing config schema는 *안 바뀜* (breaking change 아님).

필드 매핑은 *구현 시* 세부 결정. RFC는 *방향*만 명시.

#### 3.5.5 BrowserApi shutdown 처리

kernel의 `BrowserApi`는 `shutdown()` 메서드를 가진다 (graceful cleanup). oxi-sdk의 `native_browser_tools()`는 shutdown hook을 *명시 제공하지 않는다* — oxi-agent의 `OxiBrowserEngine`이 `oxibrowser_core::Browser`를 소유하므로, 마지막 `ToolRegistry` 참조가 drop될 때 자동으로 `close()`가 호출된다 (Drop impl 가정, 검증 필요). binary에 명시적 shutdown hook이 필요하면 oxi-sdk에 *PR을 올려 추가*하는 것을 원칙으로 한다 (AGENTS.md "oxi-sdk 재구현 금지" 준수).

#### 3.5.6 영향

- **`oxios_kernel::BrowserApi` 타입 *제거*** — public surface 깨짐. binary는 내부 consumer였으므로 직접 영향.
- **`oxios_kernel::BrowserTool` 타입 *제거*** — `kernel_bridge.rs::tool_names()`에서 "browser" 사라짐. agent 입장에서는 *도구 이름이 바뀜*:
  - 이전: 단일 `"browser"` tool
  - 이후: `oxi-sdk`의 `"browse"`, `"browse_extract"`, `"browse_session"`, `"browse_script"` (4개)
  - **에이전트의 tool 선택 행동이 바뀜** — 단일 generic 도구 대신 4개의 specialized 도구. prompt에 따라 자동으로 더 적합한 도구 선택. 단, *에이전트가 기존 `browser` 도구를 호출하던 prompt는 깨짐*.
- **`oxios-kernel` features에서 `browser` 제거** — root의 `browser = ["oxios-kernel/browser"]`는 `browser = ["oxi-sdk/native-browser"]`로 재정의.
- **`OxiosConfig::BrowserConfig`는 유지** — 사용자 config schema 보존. 필드 의미가 oxi-sdk 의미로 자동 매핑됨.

#### 3.5.7 CHANGELOG 항목 (breaking)

- `oxios_kernel::BrowserApi` *removed*. Use `oxi_sdk::native_browser_tools()` instead.
- `oxios_kernel::BrowserTool` *removed*. Browser capability now provided by `oxi-sdk`.
- `oxios-kernel` feature `browser` *removed*. Use root feature `browser` (which now enables `oxi-sdk/native-browser`).
- `oxios-kernel` direct dependency on `oxibrowser-core` *removed* (now transitive via `oxi-sdk`).
- Tool name migration: `browser` → `browse` / `browse_extract` / `browse_session` / `browse_script` (4 separate tools).

---

## 4. 마이그레이션 순서

작업은 *순서가 중요*하다. 한 단계가 깨지면 다음 단계가 컴파일 안 된다.

### Phase A: 명칭/잔재 정리 (1 PR, 리스크 낮음)

```
1. tools/kernel/ → tools/builtin/       (디렉터리 mv, mod.rs 1줄 수정)
2. memory/budget.rs → memory/quota.rs    (파일 mv, mod.rs 1줄 수정)
3. web/channel.rs → web_channel.rs       (파일 mv, lib.rs 1줄 + use 문 일괄)
4. src/cmd_*.rs → src/commands/          (디렉터리화, 3개 파일 수정)
5. *.md 잔재 3건 삭제                    (rm)
6. benches/kernel_bench.rs 이동          (mv + root Cargo.toml [[bench]] 제거)
7. cargo build && cargo test --workspace  ← 모든 단계 후 빌드/테스트 확인
```

이 Phase는 oxios-memory 추출과 *독립*이므로, 먼저 머지해도 안전.

### Phase B: `oxios-memory` crate 신설 (1 PR, 중간 리스크)

```
1. crates/oxios-memory/ 디렉터리 생성, Cargo.toml 작성
2. 32개 .rs 파일 이동 (memory/, embedding.rs, embedding/gguf/)
3. lib.rs 작성: 모든 공개 타입 re-export
4. crates/oxios-kernel/Cargo.toml:
     + oxios-memory = { path = "../oxios-memory", version = "1.0.2" }
     - features: sqlite-memory, embedding-gguf 제거
     - sqlite/gguf 관련 직접 의존 정리
5. crates/oxios-kernel/src/lib.rs:
     pub use memory::{...}  →  pub use oxios_memory::{...}
     pub use embedding::{...} → pub use oxios_memory::embedding::{...}
6. crates/oxios-kernel/src 내부 use 문 일괄:
     use crate::memory::X   →  use oxios_memory::X
     use crate::embedding::Y →  use oxios_memory::embedding::Y
7. kernel_handle/agent_api.rs에서 memory_manager 필드 유지 (Arc<oxios_memory::MemoryManager>로 타입만 변경)
8. src/kernel.rs: use oxios_kernel::MemoryManager 등 그대로 (re-export로 동작)
9. channels/oxios-cli, surface/oxios-web: use oxios_kernel::MemoryManager 그대로
10. cargo build && cargo test --workspace
```

### Phase C: KernelHandle `MemoryApi` (1 PR, 표면 변경)

```
1. kernel_handle/memory_api.rs 신설
2. AgentApi에서 memory_manager/hnsw_index 필드 제거 → memory: MemoryApi 통합
3. KernelHandle::memory() accessor 추가
4. kernel_handle/mod.rs pub use
5. 외부 consumer가 `handle.agent().recall()` 호출했다면 → `handle.memory().recall()`로 안내 (CHANGELOG)
6. cargo test
```

### Phase D: Default feature 정리 (1 PR, 빌드 매트릭스 검증 필요)

```
1. oxios-kernel: features에서 browser 외 모두 제거
2. oxios-memory: features default = ["sqlite"] 설정
3. 루트 Cargo.toml: default = ["web", "cli", "browser"], sqlite-memory 제거
4. CI 매트릭스 검증:
   cargo build --no-default-features --features cli
   cargo build --no-default-features --features web
   cargo build
5. AGENTS.md의 "Quick Facts" 표 업데이트
```

---

## 5. 영향 분석

### 5.1 변경되는 import 표면

| 경로 | 변경 |
|------|------|
| `oxios_kernel::MemoryManager` | → `oxios_memory::MemoryManager` (kernel이 re-export하므로 기존 코드도 동작) |
| `oxios_kernel::memory::store::HnswMemoryIndex` | → `oxios_memory::memory::store::HnswMemoryIndex` (동일하게 re-export) |
| `oxios_kernel::EmbeddingProvider` | → `oxios_memory::embedding::EmbeddingProvider` (kernel이 re-export) |
| `oxios_kernel::tools::kernel::agent_tool` | → `oxios_kernel::tools::builtin::agent_tool` (필드 경로만 변경) |
| `oxios_kernel::memory::budget::MemoryBudget` | → `oxios_memory::memory::quota::MemoryBudget` (파일명만 변경, 타입은 동일) |
| `oxios_web::channel::WebChannel` | → `oxios_web::web_channel::WebChannel` |
| `oxios_kernel::features::sqlite-memory` | → `oxios_memory::features::sqlite` |
| `oxios_kernel::features::embedding-gguf` | → `oxios_memory::features::gguf` |

### 5.2 영향받는 파일 (정확한 카운트)

명령으로 확인한 의존처 (Phase B 시 함께 수정):

```
# crate::memory를 import하는 kernel 내부 파일 (memory/ 내부 8개 제외)
agent_runtime.rs
auto_memory_bridge.rs       (memory/에서 kernel 루트로 승격, §3.1.1)
kernel_handle/agent_api.rs
kernel_handle/knowledge_lens.rs
kernel_handle/mod.rs
lib.rs
project/manager.rs
session_context.rs
supervisor.rs
tools/kernel_bridge.rs
tools/memory_tools.rs
embedding.rs          (kernel의 embedding → oxios_memory::embedding)
```
→ **12개 파일**에서 import 경로 갱신 필요. `auto_memory_bridge.rs`는 이동과 동시에 `use super::*` → `use oxios_memory::*`로 변경.

```
# crate::embedding를 import하는 kernel 내부 파일 (embedding/ 내부 1개 제외)
lib.rs
memory/mod.rs          (memory/는 oxios-memory로 이동하면서 함께 정리)
memory/sona.rs
memory/sqlite_store.rs
memory/store.rs
tools/retrieval.rs
```
→ Phase B에서 5개는 oxios-memory *내부*로 이동 (자동 해결). kernel에 남는 것은 `lib.rs`(re-export)와 `tools/retrieval.rs` 2건.

### 5.3 외부 영향 (binary, surface, channels)

- `src/kernel.rs` (binary): `use oxios_kernel::{MemoryManager, ...}` — *re-export로 동작*. 또는 명시적으로 `use oxios_memory::MemoryManager;`로 변경 가능.
- `surface/oxios-web`: memory 타입을 직접 import하지 않음 (rg 확인). `lib.rs`의 channel → web_channel rename만 영향.
- `channels/oxios-cli`, `channels/oxios-telegram`: memory 타입을 직접 import하지 않음.

**→ 외부 표면 변경은 web_channel rename 1건뿐.**

### 5.4 테스트 영향

- `crates/oxios-kernel/tests/` (kernel 자체 통합 테스트): 0건이 `crate::memory` 직접 참조 (rg 확인). 모두 `oxios_kernel::*` 외부 경로 사용.
- `crates/oxios-ouroboros/tests/`: memory 미사용.
- `crates/oxios-gateway/tests/`: memory 미사용.
- `tests/`(워크스페이스 루트): E2E 테스트, 영향 없음.

**→ 테스트 코드 추가 수정 불필요.**

### 5.5 Browser 일원화 (Phase E) 영향

**제거되는 public 표면:**

```rust
// 다음 타입들이 oxios-kernel에서 사라진다:
oxios_kernel::BrowserApi                // kernel_handle::BrowserApi
oxios_kernel::BrowserTool               // tools::browser::BrowserTool
oxios_kernel::features::browser         // Cargo feature
oxios_kernel::BrowserConfig             // config::BrowserConfig → oxi-sdk의 BrowseConfig로 대체
oxios_kernel::BrowserApi::from_config   // factory
oxios_kernel::BrowserApi::browser       // accessor
oxios_kernel::BrowserApi::shutdown      // graceful cleanup
```

**추가되는 호출 surface (binary → oxi-sdk):**

```rust
// binary의 src/kernel.rs에서:
oxi_sdk::native_browser_tools_with_config(browse_config)  // 도구 세트 생성
oxi_sdk::AgentBuilder::native_browser()                   // agent builder에 등록
```

**도구 이름 변경 (breaking for agents):**

| 이전 | 이후 |
|------|------|
| `browser` (단일, 다목적) | `browse` (단순 read) |
|                          | `browse_extract` (구조화 추출) |
|                          | `browse_session` (영구 세션) |
|                          | `browse_script` (YAML 스크립트) |

**config schema:** 변경 없음 (사용자 facing `~/.oxios/config.toml`의 `[browser]` 섹션은 그대로). 단, `browser.engine.*` 필드명이 oxi-sdk의 `BrowseConfig`로 자동 매핑됨. *구현 시* 매핑 테이블 명세.

**의존성 그래프 변화:**

```diff
  oxios-kernel:
-   oxibrowser-core = "0.9.1"  # direct
-   [features]
-   browser = []
-   native-browser = ["oxi-sdk/native-browser"]
+   # browser 관련 모든 의존성 제거
+
+  루트 oxios:
+   [features]
+   browser = ["oxi-sdk/native-browser"]  # 의미 변경: kernel의 것이 아니라 oxi-sdk의 것
```

**AGENTS.md 위반 해소:**
- "oxi-sdk 재구현 금지" 위반 → 해소.
- oxios의 browser 코드는 *설정 매핑 함수*와 *config 스키마 유지*만 남는다. 실제 browser 로직은 oxi-sdk가 *유일한* 소유자.

---

## 6. 리스크

### 6.1 build 캐시 무효화

`oxios-kernel`에서 14k LoC가 빠지므로 **전체 의존 crate의 incremental cache가 무효화**된다. 첫 빌드는 cold compile로 수 분이 걸릴 수 있다.

**완화:** Phase A(명칭 정리)와 Phase B(memory 추출)를 *같은 PR*로 묶지 말고 분리. Phase A가 머지된 후 `cargo clean` 없이 Phase B 진행하면 작은 crate 순서로 컴파일러가 워밍업된다.

### 6.2 의존성 누락

memory/ 가 의존하는 외부 crate 중 kernel에 의해 *우연히* import되던 것이 있을 수 있다 (예: `chrono-tz`는 `oxios-markdown`에 있고, memory에서 시간대 사용 시 어떻게 되는지).

**완화:** `crates/oxios-memory/Cargo.toml`에 `chrono-tz` 등 누락 의존성을 일찍 발견하도록 Phase B 시작 시 `cargo build`를 *빈 의존* 상태로 한 번 실행해 누락 컴파일 에러를 수집.

### 6.3 `memory_tools.rs` (kernel에 남음)

`tools/memory_tools.rs`는 kernel의 tool bridge에 등록되는 memory-관련 tool이다. 이건 kernel에 남는다 (memory crate에 두면 tool 등록이 두 곳으로 나뉨). 다만 이 tool은 `oxios_memory::*`를 import하므로 import 경로만 갱신.

**완화:** Phase B 단계 9에서 일괄 처리.

### 6.4 `AutoMemoryBridge`

`memory/auto_memory_bridge.rs`는 knowledge(`oxios_markdown`) ↔ memory 양방향 동기화. memory가 oxios-memory로 이동하면 **oxios-memory는 oxios-markdown을 의존해야 하는가?**

**결론: 아니다.** `AutoMemoryBridge`는 memory의 *사용자 코드*(kernel이 조립)다. 이걸 oxios-memory가 직접 import하면 *설계 위반*:

- (a) bridge는 *orchestration* 로직이지 memory 저장 로직이 아님.
- (b) oxios-memory는 *데이터 layer*로 단일 책임을 유지해야 함 (markdown은 별개 도메인).
- (c) kernel이 bridge를 소유하는 것이 자연스러움 — knowledge와 memory 둘 다 알고 있는 위치.

**결정:** `auto_memory_bridge.rs`는 *kernel에 남긴다* (§3.1.1 예외 1건). `memory/auto_memory_bridge.rs` → `auto_memory_bridge.rs` (kernel 루트로 *승격*). `memory/mod.rs`의 `pub mod auto_memory_bridge;` 라인 제거.

### 6.5 embedding-gguf의 aarch64 한정

현재 `embedding-gguf` feature는 `oxibrowser-core` (aarch64 전용)를 의존. 이 의존도 oxios-memory로 따라간다. **x86_64 사용자는 GGUF 못 씀**이라는 의미는 변함 없음. oxibrowser-core가 일반화되면 추후 풀림.

---

## 7. 비고

### 7.1 oxios-kernel의 무게

본 RFC 적용 후:
- kernel: ~39,500 LoC (47% of workspace)
- memory: ~14,000 LoC (17%)
- web surface: ~10,400 LoC (12%)
- markdown: ~7,500 LoC (9%)
- ouroboros, mcp, gateway, channels: 나머지

**kernel이 여전히 가장 크지만**, memory가 *독립 crate*로 분리되어 *책임이 명시*됐다. 더 이상 "kernel 안에 memory라는 거대한 블랙홀이 숨어 있는" 구조가 아니다.

### 7.2 Ouroboros의 위치

Ouroboros는 *leaf crate*다 (kernel이 ouroboros를 호출). 이름은 "the brain"으로 마케팅되지만, *의존 그래프 상*으로는 *프로토콜 라이브러리*다. 이 불일치는 AGENTS.md에 명시적으로 적혀 있지는 않지만, 본 RFC에서는 *의도적*으로 손대지 않는다 — 원본 프로젝트 attribution 보존.

### 7.3 미래 확장

- 만약 추후에 **tools/ 또는 skill/ 도 분리**하고 싶다면, 본 RFC의 Phase A 패턴(명칭 정리 → 분할)이 그대로 적용된다.
- 만약 **oxios-mcp를 kernel에 흡수**하고 싶다면, 별도 RFC로 분리 권장. MCP는 *전송 계층*이지 memory처럼 *데이터 layer*가 아니어서 흡수 동기가 다르다.

### 7.4 작업 예상

- Phase A (명칭 정리): 1일
- Phase B (oxios-memory 추출): 2일
- Phase C (MemoryApi): 1일
- Phase D (default feature): 0.5일
- Phase E (Browser 일원화): 1.5일 (제거 + oxi-sdk 통합 + config 매핑 + e2e 검증)
- **합계: ~6일** (테스트/CI 디버깅 포함)

각 Phase는 독립 PR로 머지 가능. 순서대로 머지하지 않아도 되지만, **Phase A는 Phase B 이전에** 끝내야 *경로 일관성*이 보장된다.

### 7.5 *삭제됨* — §7.6으로 대체

(이전 진단에서 §7.5는 "`browser` feature의 no-op이 bug이므로 `oxi-sdk/native-browser`로 연결하면 silent fix가 된다"고 주장했으나, 사용자 질문과 코드 검증으로 이 가정이 *틀렸음*이 확인되어 §7.6 *Browser 일원화 (Phase E)* 으로 대체한다.)

### 7.6 Browser 일원화 (Phase E) — 결정 기록

> **이 섹션은 *본 RFC의 일부* (Phase E, §3.5)의 결정 근거를 적는다.** 이전 섹션(§7.5)에서는 "browser는 별도 RFC로" 제안했으나, 본 RFC에 *흡수*하는 것으로 결정이 *역전*되었다 (2026-06-04 사용자 결정).

#### 검증된 사실 (코드 재검증, 2026-06-04)

**oxi-sdk는 자체 browser 도구 세트를 가지고 있다:**

```
oxi-sdk/src/agent_builder.rs
    native_browser(self) -> Self
    native_browser_tools_with_config(...)
oxi-sdk/src/tool_factory.rs
    native_browser_tools() -> Arc<ToolRegistry>
    native_browser_tools_with_config(...)
oxi-agent/src/tools/browse/
    browse_tool.rs           → "browse"
    browse_extract_tool.rs   → "browse_extract"
    browse_session_tool.rs   → "browse_session"
    oxibrowser_backend.rs    → OxiBrowserEngine (oxibrowser-core 래퍼)
```

**oxios-kernel은 *별도의* browser 도구를 가진다:**

```
oxios-kernel/src/tools/browser/browser_tool.rs   → 단일 "browser" tool
oxios-kernel/src/kernel_handle/browser_api.rs    → BrowserApi (lazy init, lifecycle)
```

**둘 다 `oxibrowser-core`를 백엔드로 사용한다.** 차이:

| 측면 | oxi-sdk의 native browser | oxios-kernel의 BrowserTool |
|------|--------------------------|----------------------------|
| 등록 tool 이름 | `browse`, `browse_extract`, `browse_session` (다수) | `browser` (단일, 다목적) |
| 백엔드 엔진 | `OxiBrowserEngine` (oxi-agent) | `oxibrowser_core::Browser` (직접) |
| 게이트 feature | `oxi-sdk/native-browser` | `oxios-kernel/browser` (현재 no-op) |
| lifecycle 관리 | oxi-agent 내부 | kernel의 `BrowserApi` |
| 설정 노출 | `oxi_sdk::BrowseConfig` | `oxios_kernel::BrowserConfig` |

**현재 상태:** `kernel_bridge.rs::tool_names()`는 `"browser"` (kernel 버전) *하나만* 등록. `oxi-sdk::native_browser_tools()`는 *어디에서도 호출되지 않는다* (rg 확인). oxi-sdk의 browse/browse_extract/browse_session은 *dead path*.

#### AGENTS.md 위반

AGENTS.md는 "oxi-sdk 재구현 금지"라고 못 박는다. oxios-kernel의 `BrowserTool`/`BrowserApi`는 oxi-sdk의 `native_browser_tools()`를 우회하고 *자체 구현*한다. **이는 위반**.

#### 결정 (사용자 결정, 2026-06-04)

> **옵션 (a) 채택: oxi-sdk의 browser 도구로 일원화.**
> - kernel의 `BrowserTool`/`BrowserApi`/`tools/browser/`/`kernel_handle/browser_api.rs`를 *완전히 제거*한다.
> - binary의 `KernelBuilder::build_browser_api()`는 oxi-sdk의 `native_browser_tools()` / `AgentBuilder::native_browser()` 호출로 대체한다.
> - `oxibrowser-core`는 *transitive* 의존성으로만 남는다 (oxi-sdk 경유).
> - root의 `browser` feature는 `["oxi-sdk/native-browser"]`로 *재정의*.
> - **AGENTS.md 위반 해소**.

옵션 (b)/(c)는 *기각* — (b)는 위반 해소 안 됨, (c)는 분담 정당화 비용이 (a)의 이점보다 큼.

---

## 8. 체크리스트 (PR 작성 시)

### Phase A
- [ ] `tools/kernel/` → `tools/builtin/` 디렉터리 rename
- [ ] `tools/mod.rs`의 `mod kernel;` → `mod builtin;`
- [ ] `memory/budget.rs` → `memory/quota.rs` 파일 rename
- [ ] `memory/mod.rs`의 `mod budget;` → `mod quota;`
- [ ] `web/src/channel.rs` → `web/src/web_channel.rs`
- [ ] `web/src/lib.rs`의 `mod channel;` → `mod web_channel;`
- [ ] `web/src/plugin.rs`, `web/src/routes/*.rs`에서 `use crate::channel` 일괄 치환
- [ ] `src/cmd_run.rs`, `src/cmd_update.rs` → `src/commands/{run,update}.rs`
- [ ] `src/commands/mod.rs` 신설
- [ ] `src/main.rs`의 `mod cmd_run; mod cmd_update;` → `mod commands;`
- [ ] `*.md` 3건 삭제 (retrieval-output, kernel/impl-output, kernel_handle/impl-output)
- [ ] `benches/kernel_bench.rs` → `crates/oxios-kernel/benches/state_store.rs`
- [ ] `benches/` 디렉터리 삭제
- [ ] 루트 `Cargo.toml`의 `[[bench]]` 섹션 삭제
- [ ] `cargo build && cargo test --workspace` 통과

### Phase B
- [ ] `crates/oxios-memory/` 디렉터리 생성
- [ ] `Cargo.toml` 작성 (Section 3.1.3 참고)
- [ ] 31개 .rs 파일을 `crates/oxios-memory/src/`로 이동 (`git mv` 사용, auto_memory_bridge.rs 제외)
- [ ] `memory/auto_memory_bridge.rs`를 `crates/oxios-kernel/src/auto_memory_bridge.rs`로 이동, `use super::*` → `use oxios_memory::*`
- [ ] `memory/mod.rs`에서 `pub mod auto_memory_bridge;` 라인 제거
- [ ] `oxios-memory/src/lib.rs` 작성 (memory, embedding, gguf re-export)
- [ ] kernel `Cargo.toml`에 `oxios-memory` path dep 추가
- [ ] kernel `Cargo.toml`에서 `sqlite-memory`, `embedding-gguf` feature 제거
- [ ] kernel `lib.rs` re-export를 `oxios_memory`로 변경
- [ ] kernel 내부 `use crate::memory::*` → `use oxios_memory::*` 일괄 (12개 파일)
- [ ] `tools/retrieval.rs`의 `use crate::embedding::*` → `use oxios_memory::embedding::*`
- [ ] `src/kernel.rs`의 import 검증 (re-export로 동작)
- [ ] `cargo build && cargo test --workspace` 통과
- [ ] `criterion` bench가 새 경로에서 동작하는지 확인

### Phase C
- [ ] `kernel_handle/memory_api.rs` 신설
- [ ] `AgentApi`에서 memory 필드 제거, `MemoryApi` 통합
- [ ] `KernelHandle::memory()` accessor
- [ ] `kernel_handle/mod.rs`의 `pub use`에 MemoryApi 추가
- [ ] `CHANGELOG.md`에 breaking change 항목 추가 (`agent().memory_manager` → `memory().recall` 등)
- [ ] `cargo test` 통과

### Phase D
- [ ] `oxios-kernel/Cargo.toml` features 정리
- [ ] `oxios-memory/Cargo.toml` features 설정 (default = ["sqlite"])
- [ ] 루트 `Cargo.toml` default에서 `sqlite-memory` 제거
- [ ] CI 매트릭스: 3가지 빌드 (`default`, `--no-default-features --features cli`, `--no-default-features --features web`) 검증
- [ ] `AGENTS.md` "Quick Facts" 표 업데이트
- [ ] `README.md`의 빌드 명령어 섹션 업데이트

> **Phase D + E는 같은 PR로 묶을 것을 *권장*.** root의 `browser` feature 재정의는 *두 Phase가 동시에* 적용되어야 default가 깨지는 사이 시간이 없다 (§3.5 / §3.4).

### Phase E
- [ ] `crates/oxios-kernel/src/tools/browser/` 디렉터리 삭제 (mod.rs, browser_tool.rs)
- [ ] `crates/oxios-kernel/src/kernel_handle/browser_api.rs` 삭제
- [ ] `crates/oxios-kernel/src/lib.rs`에서 `BrowserTool`, `BrowserApi` re-export 제거
- [ ] `crates/oxios-kernel/src/tools/mod.rs`에서 `pub mod browser;` 제거
- [ ] `crates/oxios-kernel/src/tools/kernel_bridge.rs`에서 `"browser"` in `tool_names()` 제거 + register_tools에서 browser 등록 코드 제거
- [ ] `crates/oxios-kernel/src/tools/registration.rs`에서 browser 관련 제거
- [ ] `crates/oxios-kernel/Cargo.toml`에서 `oxibrowser-core` direct dep 제거
- [ ] `crates/oxios-kernel/Cargo.toml`에서 `browser` feature 제거
- [ ] `src/kernel.rs`의 `build_browser_api()` / `build_browser_api_value()` 제거
- [ ] `src/kernel.rs`에 `oxi_sdk::native_browser_tools_with_config()` 또는 `AgentBuilder::native_browser()` 호출 추가
- [ ] `OxiosConfig::BrowserConfig` → `oxi_sdk::BrowseConfig` 매핑 함수 작성
- [ ] `kernel_handle/mod.rs`에서 browser 관련 accessor 제거 (있을 경우)
- [ ] 루트 `Cargo.toml`의 `browser = ["oxi-sdk/native-browser"]` 재정의
- [ ] `cargo build && cargo test --workspace` 통과
- [ ] e2e: agent가 `browse`/`browse_extract` 도구 호출 검증 (config.browser.enabled true/false 양쪽)
- [ ] `config.toml`의 `browser.engine.*` 필드 매핑이 정상 동작 확인
- [ ] `CHANGELOG.md`에 breaking change 4건 추가

### 문서
- [ ] `docs/ARCHITECTURE.md`에 새 의존 그래프 반영
- [ ] `DESIGN.md`에 "memory는 독립 crate" 명시
- [ ] `CHANGELOG.md`에 항목 추가
- [ ] **CHANGELOG 항목:**
  - `oxios-kernel` 1.1.0: `sqlite-memory`, `embedding-gguf` features removed; use `oxios-memory`'s `sqlite`, `gguf`
  - `oxios-kernel` 1.1.0: `MemoryManager`, `EmbeddingProvider` 등 re-export 형태로 유지 (외부 호환)
  - `oxios-kernel` 1.1.0 (Phase C): `kernel_handle::AgentApi::memory_manager()` accessor *removed* (breaking)
  - `oxios-kernel` 1.1.0 (Phase E): `BrowserApi`, `BrowserTool` types *removed* (breaking). Browser now provided by `oxi-sdk::native_browser_tools()`. Tool name migration: `browser` → `browse` / `browse_extract` / `browse_session` / `browse_script`. `oxibrowser-core` direct dep removed.
