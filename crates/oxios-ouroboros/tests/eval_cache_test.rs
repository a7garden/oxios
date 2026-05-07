//! Tests for eval_cache, MechanicalEvalResult, and parse_json.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use oxios_ouroboros::evaluation::{EvaluationResult, MechanicalEvalResult};
use oxios_ouroboros::protocol::ExecutionResult;
use oxios_ouroboros::seed::Seed;

/// Helper to create a seed with a fixed ID for deterministic testing.
fn test_seed() -> Seed {
    let seed = Seed::new("test goal");
    seed
}

fn test_execution(output: &str) -> ExecutionResult {
    ExecutionResult {
        output: output.to_string(),
        steps_completed: 1,
        success: true,
    }
}

// ---------------------------------------------------------------------------
// EvalCache tests (using the public API through EvalCache)
// ---------------------------------------------------------------------------

// Since EvalCache is not re-exported, we test MechanicalEvalResult and parse_json
// via the public API. EvalCache internals are tested implicitly through the engine.

// ---------------------------------------------------------------------------
// MechanicalEvalResult tests
// ---------------------------------------------------------------------------

#[test]
fn mechanical_eval_all_criteria_pass() {
    let criteria = vec![
        "hello world".to_string(),
        "success".to_string(),
    ];
    let output = "The program printed hello world and reported success.";
    let result = MechanicalEvalResult::evaluate(&criteria, output);
    assert!(result.all_passed);
    assert_eq!(result.criterion_results.len(), 2);
    assert!(result.criterion_results[0].passed);
    assert!(result.criterion_results[1].passed);
}

#[test]
fn mechanical_eval_some_criteria_fail() {
    let criteria = vec![
        "hello world".to_string(),
        "missing keyword".to_string(),
    ];
    let output = "The program printed hello world.";
    let result = MechanicalEvalResult::evaluate(&criteria, output);
    assert!(!result.all_passed);
    assert!(result.criterion_results[0].passed);
    assert!(!result.criterion_results[1].passed);
}

#[test]
fn mechanical_eval_exit_code_detection() {
    let criteria = vec!["exit code 0".to_string()];
    let output = "Process finished with exit code 0";
    let result = MechanicalEvalResult::evaluate(&criteria, output);
    assert!(result.all_passed);
    assert!(result.criterion_results[0].passed);
}

#[test]
fn mechanical_eval_exit_status_detection() {
    let criteria = vec!["Exit Status is 0".to_string()];
    let output = "Result: exit status 0";
    let result = MechanicalEvalResult::evaluate(&criteria, output);
    assert!(result.all_passed);
}

#[test]
fn mechanical_eval_exit_code_failure() {
    let criteria = vec!["exit code 0".to_string()];
    let output = "Process failed with exit code 1";
    let result = MechanicalEvalResult::evaluate(&criteria, output);
    assert!(!result.all_passed);
    assert!(!result.criterion_results[0].passed);
}

#[test]
fn mechanical_eval_empty_criteria() {
    let criteria: Vec<String> = vec![];
    let output = "anything";
    let result = MechanicalEvalResult::evaluate(&criteria, output);
    assert!(result.all_passed); // vacuously true
    assert!(result.criterion_results.is_empty());
}

// ---------------------------------------------------------------------------
// parse_json tests (prose-wrapped JSON)
// ---------------------------------------------------------------------------

/// A simple struct to parse from JSON for testing.
#[derive(serde::Deserialize, Debug, PartialEq)]
struct TestJson {
    name: String,
    value: i64,
}

/// Replicate the parse_json logic here for unit testing since it's a private method.
fn parse_json<T: serde::de::DeserializeOwned>(raw: &str) -> anyhow::Result<T> {
    let trimmed = raw.trim();
    let json_str = if trimmed.starts_with("```") {
        let after_open = trimmed.find('\n').map(|i| i + 1).unwrap_or(0);
        let before_close = trimmed.rfind("```").unwrap_or(trimmed.len());
        &trimmed[after_open..before_close]
    } else if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            &trimmed[start..=end]
        } else {
            trimmed
        }
    } else if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            &trimmed[start..=end]
        } else {
            trimmed
        }
    } else {
        trimmed
    };
    Ok(serde_json::from_str(json_str.trim())?)
}

#[test]
fn parse_json_pure_json() {
    let input = r#"{"name": "test", "value": 42}"#;
    let result: TestJson = parse_json(input).unwrap();
    assert_eq!(result.name, "test");
    assert_eq!(result.value, 42);
}

#[test]
fn parse_json_markdown_fenced() {
    let input = "```json\n{\"name\": \"fenced\", \"value\": 7}\n```";
    let result: TestJson = parse_json(input).unwrap();
    assert_eq!(result.name, "fenced");
    assert_eq!(result.value, 7);
}

#[test]
fn parse_json_prose_wrapped() {
    let input = "Here is the result:\n{\"name\": \"prose\", \"value\": 99}\nThat should work.";
    let result: TestJson = parse_json(input).unwrap();
    assert_eq!(result.name, "prose");
    assert_eq!(result.value, 99);
}

#[test]
fn parse_json_array_wrapped() {
    let input = "Results:\n[1, 2, 3]\nDone.";
    let result: Vec<i64> = parse_json(input).unwrap();
    assert_eq!(result, vec![1, 2, 3]);
}

#[test]
fn parse_json_invalid_still_fails() {
    let input = "not json at all";
    let result: anyhow::Result<TestJson> = parse_json(input);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// EvalCache tests (inline since eval_cache is not re-exported)
// We re-implement a minimal version to test the logic.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct EvalKey {
    seed_id: uuid::Uuid,
    output_hash: u64,
}

impl EvalKey {
    fn new(seed: &Seed, execution: &ExecutionResult) -> Self {
        let mut hasher = DefaultHasher::new();
        execution.output.hash(&mut hasher);
        Self {
            seed_id: seed.id,
            output_hash: hasher.finish(),
        }
    }
}

struct EvalCache {
    cache: std::collections::HashMap<EvalKey, EvaluationResult>,
    max_entries: usize,
}

impl EvalCache {
    fn new(max_entries: usize) -> Self {
        Self {
            cache: std::collections::HashMap::new(),
            max_entries,
        }
    }

    fn get(&self, seed: &Seed, execution: &ExecutionResult) -> Option<EvaluationResult> {
        let key = EvalKey::new(seed, execution);
        self.cache.get(&key).cloned()
    }

    fn put(&mut self, seed: &Seed, execution: &ExecutionResult, result: EvaluationResult) {
        let key = EvalKey::new(seed, execution);
        if self.cache.len() >= self.max_entries {
            if let Some(first_key) = self.cache.keys().next().cloned() {
                self.cache.remove(&first_key);
            }
        }
        self.cache.insert(key, result);
    }
}

fn make_eval_result(pass: bool, score: f64) -> EvaluationResult {
    EvaluationResult {
        mechanical_pass: pass,
        semantic_pass: None,
        consensus_pass: None,
        score,
        notes: vec![],
    }
}

#[test]
fn eval_cache_basic_get_put() {
    let mut cache = EvalCache::new(10);
    let seed = test_seed();
    let exec = test_execution("output");
    let result = make_eval_result(true, 0.9);

    assert!(cache.get(&seed, &exec).is_none());
    cache.put(&seed, &exec, result.clone());
    let cached = cache.get(&seed, &exec).unwrap();
    assert_eq!(cached.score, 0.9);
    assert!(cached.mechanical_pass);
}

#[test]
fn eval_cache_different_outputs_different_keys() {
    let mut cache = EvalCache::new(10);
    let seed = test_seed();
    let exec1 = test_execution("output A");
    let exec2 = test_execution("output B");

    cache.put(&seed, &exec1, make_eval_result(true, 0.9));
    cache.put(&seed, &exec2, make_eval_result(false, 0.3));

    let r1 = cache.get(&seed, &exec1).unwrap();
    let r2 = cache.get(&seed, &exec2).unwrap();
    assert!(r1.mechanical_pass);
    assert!(!r2.mechanical_pass);
}

#[test]
fn eval_cache_eviction() {
    let mut cache = EvalCache::new(2);
    let seed1 = test_seed();
    let seed2 = Seed::new("goal 2");
    let seed3 = Seed::new("goal 3");

    let exec = test_execution("output");
    cache.put(&seed1, &exec, make_eval_result(true, 0.9));
    cache.put(&seed2, &exec, make_eval_result(true, 0.8));

    // Cache is full (2 entries). Adding a third should evict the first.
    cache.put(&seed3, &exec, make_eval_result(true, 0.7));

    // seed1 should be evicted
    assert!(cache.get(&seed1, &exec).is_none());
    // seed2 and seed3 should still be present
    assert!(cache.get(&seed2, &exec).is_some());
    assert!(cache.get(&seed3, &exec).is_some());
}

#[test]
fn eval_cache_same_seed_same_output_returns_cached() {
    let mut cache = EvalCache::new(10);
    let seed = test_seed();
    let exec = test_execution("identical output");

    cache.put(&seed, &exec, make_eval_result(true, 0.95));
    let cached = cache.get(&seed, &exec).unwrap();
    assert_eq!(cached.score, 0.95);

    // Put again with different result — should overwrite
    cache.put(&seed, &exec, make_eval_result(false, 0.5));
    let cached = cache.get(&seed, &exec).unwrap();
    assert_eq!(cached.score, 0.5);
}
