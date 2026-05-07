# Lightweight Protocol: Fast-Track Agent Execution

> **문제:** Ouroboros의 interview phase가 간단한 작업에 과도함  
> **해결:** Confidence-based routing — 복잡도 따라 경량/표준 경로 자동 선택

---

## 1. Problem: The Ouroboros Tax

### 현재 파이프라인 (항상)

```
사용자: "fix the bug in main.rs"
  ↓
Phase 1: Interview (LLM call)
  → "Questions to clarify..." (지연 + 비용)
  ↓ (모호성 ≤ 0.2)
Phase 2: Seed Generation (LLM call)
  → Structured spec
  ↓
Phase 3: Execute (AgentLoop)
  ↓
Phase 4: Evaluate (LLM call)
  ↓
Phase 5: Evolve (optional, LLM call)
```

**비용:** 3-5 LLM calls, 5-30초 지연  
**문제:** 간단한 "이 파일 고쳐줘" 에게 과도함

### 해결: Dual-Track Protocol

```
                    ┌─ confidence high? ──→ Lightweight (skip interview)
사용자 메시지 ──→ Router
                    └─ confidence low? ──→ Ouroboros (full spec-first)
```

**원칙:** 복잡도는 시스템이 판단. 사용자는 하나만 입력.

---

## 2. Design: Confidence Scorer

### 2.1 Confidence Indicators

사용자 입력이 다음 조건을 만족하면 **high confidence**:

```
✅ Low Complexity
  - 파일 경로가 포함됨 (read/write/fix specific file)
  - 명령어가 구체적 ("grep", "sed", "cargo test")
  - 의도가 명확 ("고쳐", "추가해", "삭제해")
  - acceptance criteria가 암묵적 ("작동하면 된다")

✅ Short Input
  - 200자 이하
  - 의문사가 없음 (who/why/what should I... 없음)
  - 질문 없음

✅ No Ambiguity Markers
  - "어떻게", "무엇을", "왜" 없음
  - "이거랑 저거 관련" 없음
  - 다중 작업 없음
```

### 2.2 Confidence Score Formula

```rust
pub struct ConfidenceScore {
    /// 0.0 (very ambiguous) to 1.0 (very clear)
    score: f64,
    /// Why we scored this way
    reasons: Vec<ConfidenceReason>,
}

#[derive(Debug, Clone)]
enum ConfidenceReason {
    HasFilePath(String),           // +0.2
    IsShortInput(usize),           // +0.1 if < 200 chars
    HasSpecificVerb(String),      // +0.15 "fix", "add", "remove", "update"
    NoQuestionMarkers,            // +0.15
    HasAcceptanceImplicit,        // +0.1 "work", "run", "pass"
    HasAmbiguousMarkers,          // -0.3 "something", "maybe", "or"
    MultipleTasks,                // -0.2 (2+ distinct goals)
    VagueGoal,                    // -0.3 no specific target
}

impl ConfidenceScore {
    pub fn analyze(input: &str) -> Self {
        let mut score = 0.5;  // baseline
        let mut reasons = Vec::new();

        // Positive signals
        if Self::has_file_path(input) {
            score += 0.2;
            reasons.push(ConfidenceReason::HasFilePath("detected".into()));
        }
        if input.len() < 200 {
            score += 0.1;
        }
        if Self::has_specific_verb(input) {
            score += 0.15;
        }
        if !Self::has_question_markers(input) {
            score += 0.15;
        }
        if Self::has_implicit_acceptance(input) {
            score += 0.1;
        }

        // Negative signals
        if Self::has_ambiguous_markers(input) {
            score -= 0.3;
        }
        if Self::has_multiple_tasks(input) {
            score -= 0.2;
        }
        if Self::is_vague(input) {
            score -= 0.3;
        }

        // Clamp to [0.0, 1.0]
        score = score.clamp(0.0, 1.0);

        Self { score, reasons }
    }

    /// True if score >= threshold → use lightweight path
    pub fn is_high_confidence(&self) -> bool {
        self.score >= 0.7
    }
}
```

### 2.3 Examples

| Input | Score | Path |
|-------|-------|------|
| `"fix the null pointer in main.rs:42"` | 0.95 | **Lightweight** |
| `"add authentication to the login endpoint"` | 0.85 | **Lightweight** |
| `"cargo test in the auth module"` | 0.90 | **Lightweight** |
| `"make the app faster somehow"` | 0.30 | **Ouroboros** |
| `"why is the API slow? can you also check the DB?"` | 0.35 | **Ouroboros** |
| `"we need to improve the overall user experience"` | 0.20 | **Ouroboros** |
| `"review the codebase and suggest improvements"` | 0.25 | **Ouroboros** |
| `"test the auth module, fix bugs, update docs"` | 0.15 | **Ouroboros** (3 tasks) |

---

## 3. Lightweight Path

### 3.1 Direct to Seed (no interview)

```rust
/// Lightweight path: skips interview, generates seed directly
async fn handle_lightweight(&self, user_id: &str, input: &str) -> Result<OrchestrationResult> {
    // Step 1: Generate seed directly from input (no clarification)
    let seed = self.ouroboros.generate_seed_lightweight(input).await?;

    // Step 2: Execute (same as Ouroboros path)
    let exec_result = self.lifecycle.spawn_and_run(&seed, Priority::High).await?;

    // Step 3: Quick evaluation (just mechanical check, no LLM)
    let passed = self.mechanical_check(&seed, &exec_result);

    // Step 4: If failed, fall back to full Ouroboros
    if !passed {
        tracing::info!("Lightweight failed, falling back to Ouroboros for: {}", input);
        return self.ouroboros_handle_message(user_id, input, None).await;
    }

    Ok(OrchestrationResult {
        response: exec_result.output,
        phase_reached: Phase::Execute,
        // ... other fields
    })
}
```

### 3.2 Lightweight Seed Generation

```rust
/// Generate a seed WITHOUT the interview phase.
/// Uses a simple rule-based + single LLM call.
async fn generate_seed_lightweight(&self, input: &str) -> Result<Seed> {
    // Rule-based extraction (fast, no LLM)
    let goal = Self::extract_goal(input);
    let constraints = Self::extract_constraints(input);
    let acceptance_criteria = Self::infer_acceptance_criteria(input);

    // If goal is complex, fall back to full LLM-assisted seed gen
    if goal.len() > 500 || constraints.len() > 10 {
        // LLM call for structured extraction
        return self.generate_seed_via_llm(input).await;
    }

    // Rule-based seed (no LLM needed for simple inputs)
    Ok(Seed::lightweight(goal, constraints, acceptance_criteria))
}
```

### 3.3 Mechanical Evaluation (no LLM)

```rust
/// Fast evaluation: check acceptance criteria without LLM.
/// Returns true if output contains key success indicators.
fn mechanical_check(&self, seed: &Seed, result: &ExecutionResult) -> bool {
    let output_lower = result.output.to_lowercase();

    // Check if output mentions success indicators
    let success_indicators = [
        "done", "completed", "success", "fixed", "added", "removed",
        "updated", "passed", "ok", "no error", "no errors",
    ];

    let has_success = success_indicators
        .iter()
        .any(|ind| output_lower.contains(ind));

    // Check for error indicators
    let error_indicators = ["error", "failed", "panic", "crash", "exception"];
    let has_error = error_indicators
        .iter()
        .any(|ind| output_lower.contains(ind));

    has_success && !has_error
}
```

---

## 4. Unified Handler

### 4.1 Router Integration

```rust
pub async fn handle_message(
    &self,
    user_id: &str,
    input: &str,
    session_id: Option<&str>,
) -> Result<OrchestrationResult> {
    // Route: lightweight or full Ouroboros?
    let confidence = ConfidenceScore::analyze(input);

    if confidence.is_high_confidence() {
        tracing::info!(
            confidence = confidence.score,
            reasons = ?confidence.reasons,
            "Routing to lightweight path"
        );
        return self.handle_lightweight(user_id, input, confidence).await;
    }

    tracing::info!(
        confidence = confidence.score,
        reasons = ?confidence.reasons,
        "Routing to Ouroboros (full spec-first)"
    );
    self.ouroboros_handle_message(user_id, input, session_id).await
}
```

### 4.2 User Experience

```
User: "fix main.rs:42 null pointer"

  System: [Confidence: 0.95 — Lightweight path]
  → Seed generated in 200ms (rule-based)
  → Agent executing...
  → Completed in 3s
  → ✓ Fixed null pointer

  vs

User: "make the app better"

  System: [Confidence: 0.30 — Ouroboros]
  → I want to understand your request better:
  1. What specific area should I focus on?
  2. What does "better" mean to you? Performance? UX?
  3. Are there specific pain points you've noticed?
```

---

## 5. API Response Indication

### 5.1 Flag Which Path Was Used

```rust
#[derive(Serialize)]
pub struct OrchestrationResult {
    pub response: String,
    pub execution_path: ExecutionPath,  // NEW
    pub confidence: Option<f64>,         // NEW (if lightweight)
    pub session_id: Option<String>,
    pub seed_id: Option<Uuid>,
    pub phase_reached: Phase,
    pub evaluation_passed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub enum ExecutionPath {
    /// Skipped interview, direct seed generation
    Lightweight,
    /// Full Ouroboros spec-first protocol
    Ouroboros,
    /// Lightweight failed, fell back to Ouroboros
    LightweightFallback,
}
```

### 5.2 SSE Event

```json
{
  "type": "execution_path",
  "path": "lightweight",
  "confidence": 0.95
}
```

---

## 6. ConfidenceScore Extractor (Rule-based)

```rust
impl ConfidenceScore {
    fn has_file_path(s: &str) -> bool {
        // Detects: path/to/file, /absolute/path, *.rs, *.toml, etc.
        s.contains(".rs")
            || s.contains(".toml")
            || s.contains(".md")
            || s.contains('/')
            || s.contains(".py")
            || s.contains(".js")
            || s.contains(".ts")
    }

    fn has_specific_verb(s: &str) -> bool {
        let verbs = ["fix", "add", "remove", "delete", "update", "change",
                     "refactor", "test", "run", "build", "compile",
                     "install", "configure", "setup", "deploy"];
        s.split_whitespace()
            .any(|w| verbs.contains(&w.to_lowercase().as_str()))
    }

    fn has_question_markers(s: &str) -> bool {
        s.contains('?')
            || s.contains("how do")
            || s.contains("how can")
            || s.contains("what is")
            || s.contains("why does")
            || s.contains("should I")
            || s.contains("could you")
    }

    fn has_ambiguous_markers(s: &str) -> bool {
        let markers = ["something", "maybe", "or something", "not sure",
                       "kinda", "sort of", "etc", "and stuff"];
        markers.iter().any(|m| s.to_lowercase().contains(m))
    }

    fn has_multiple_tasks(s: &str) -> bool {
        // Count distinct verb-object pairs
        let separators = ['&', ',', ';', " and also ", " also "];
        let parts: Vec<&str> = separators
            .iter()
            .flat_map(|sep| s.split(sep))
            .collect();
        parts.len() > 2
    }

    fn is_vague(s: &str) -> bool {
        let vague_words = [
            "improve", "better", "faster", "cleaner",
            "more efficient", "refactor", "optimize",
            "somehow", "thing", "stuff", "whatever"
        ];
        let lower = s.to_lowercase();
        vague_words.iter().filter(|w| lower.contains(*w)).count() > 2
    }

    fn has_implicit_acceptance(s: &str) -> bool {
        let indicators = ["work", "run", "pass", "build", "compile", "ok"];
        let lower = s.to_lowercase();
        indicators.iter().any(|i| lower.contains(i))
    }
}
```

---

## 7. Metrics

Add to metrics.rs:

```rust
pub fn init_protocol_metrics() {
    let r = registry();

    r.counter("oxios_execution_path_lightweight_total", "Lightweight path executions", &[]);
    r.counter("oxios_execution_path_ouroboros_total", "Full Ouroboros executions", &[]);
    r.counter("oxios_execution_path_fallback_total", "Lightweight→Ouroboros fallbacks", &[]);

    r.histogram(
        "oxios_confidence_score",
        "Initial confidence score for routing",
        vec![0.0, 0.2, 0.4, 0.6, 0.7, 0.8, 0.9, 1.0],
    );
}
```

---

## 8. Configuration

```toml
[ouroboros]
# Minimum confidence score to use lightweight path
lightweight_threshold = 0.7

# Max input length for rule-based seed generation (no LLM)
rule_based_max_chars = 500

# Fallback to Ouroboros if lightweight evaluation fails
auto_fallback = true

# Mechanical check: require success indicators
mechanical_check_strict = false
```

---

## 9. Implementation Plan

```
Step 1: ConfidenceScorer (1 day)
  - New file: orouboros/src/confidence.rs
  - Rule-based scoring
  - Unit tests (20+ cases)

Step 2: Route in Orchestrator (half day)
  - Integrate confidence scoring
  - Log routing decision
  - Metrics instrumentation

Step 3: Lightweight Seed Generator (half day)
  - rule-based extraction
  - lightweight Seed::lightweight() constructor
  - fallback to LLM for complex inputs

Step 4: Mechanical Evaluator (half day)
  - Success/error indicator detection
  - LightweightFallback metric

Step 5: API Response (half day)
  - ExecutionPath enum
  - SSE event
  - Response metadata

Total: 3 days
```

---

## 10. Expected Impact

| Metric | Before | After |
|--------|--------|-------|
| Simple task latency | 10-30s | 2-5s |
| LLM calls (simple) | 3-5 | 0-1 |
| User perception | "slow for simple tasks" | "fast for simple, thorough for complex" |

**추정:** 60-70%의 실제 사용이 simple task → 40-50% latency 감소

---

## 11. Verification

### Test Cases

```rust
#[test]
fn test_confidence_high() {
    assert!(ConfidenceScore::analyze("fix the null pointer in main.rs:42").score >= 0.7);
    assert!(ConfidenceScore::analyze("add auth to /api/login").score >= 0.7);
    assert!(ConfidenceScore::analyze("cargo test in auth module").score >= 0.7);
}

#[test]
fn test_confidence_low() {
    assert!(ConfidenceScore::analyze("make it better").score < 0.7);
    assert!(ConfidenceScore::analyze("improve the user experience somehow").score < 0.7);
    assert!(ConfidenceScore::analyze("what should we do about this?").score < 0.7);
    assert!(ConfidenceScore::analyze("fix the bug and update the docs and test it").score < 0.7);
}
```

### E2E Tests

```rust
#[tokio::test]
async fn test_lightweight_path() {
    let orch = build_orchestrator().await;

    // Simple task → lightweight
    let result = orch.handle_message("user", "fix main.rs:42").await.unwrap();
    assert_eq!(result.execution_path, ExecutionPath::Lightweight);
    assert!(result.confidence.unwrap() >= 0.7);

    // Complex task → Ouroboros
    let result = orch.handle_message("user", "improve the overall architecture").await.unwrap();
    assert_eq!(result.execution_path, ExecutionPath::Ouroboros);
}
```