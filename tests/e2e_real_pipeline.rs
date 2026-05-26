//! E2E tests that exercise the full Ouroboros pipeline with a real LLM.
//!
//! Run with:
//! ```sh
//! OXIOS_E2E=1 cargo test --test e2e_real_pipeline -- --ignored
//! ```
//!
//! Requires a valid API key in the environment (e.g. `ANTHROPIC_API_KEY`).

#[cfg(test)]
mod tests {
    use oxios_ouroboros::OuroborosProtocol;
    use std::sync::Arc;

    fn should_run() -> bool {
        std::env::var("OXIOS_E2E").is_ok()
    }

    fn create_real_engine() -> Option<Arc<oxios_ouroboros::OuroborosEngine>> {
        if !should_run() {
            return None;
        }

        let model_id = std::env::var("OXIOS_MODEL").expect(
            "OXIOS_MODEL env var must be set (e.g. OXIOS_MODEL=anthropic/claude-sonnet-4-20250514)",
        );

        let oxi = oxi_sdk::OxiBuilder::new().with_builtins().build();
        let model = oxi
            .resolve_model(&model_id)
            .unwrap_or_else(|_| panic!("Model '{}' not found", model_id));
        let provider = oxi
            .create_provider(&model.provider)
            .unwrap_or_else(|_| panic!("Provider '{}' not available", model.provider));

        Some(Arc::new(oxios_ouroboros::OuroborosEngine::new(
            provider, model,
        )))
    }

    #[tokio::test]
    #[ignore]
    async fn test_full_interview_to_seed() {
        let engine =
            create_real_engine().expect("Set OXIOS_E2E=1 and ensure API key is in environment");

        let result = engine
            .interview("Write a Rust function that reverses a string")
            .await
            .expect("interview failed");
        assert!(
            result.ready_for_seed || !result.questions.is_empty(),
            "Interview should either be ready for seed or have questions"
        );

        let seed = engine.generate_seed(&result).await.expect("seed failed");
        assert!(!seed.goal.is_empty(), "Seed goal must not be empty");
        assert!(
            !seed.acceptance_criteria.is_empty(),
            "Seed must have acceptance criteria"
        );

        eprintln!("Goal: {}", seed.goal);
        eprintln!("Criteria: {:?}", seed.acceptance_criteria);
    }

    #[tokio::test]
    #[ignore]
    async fn test_evaluate_with_cache() {
        let engine = create_real_engine().expect("Set OXIOS_E2E=1");

        let seed = oxios_ouroboros::Seed {
            id: uuid::Uuid::new_v4(),
            goal: "Write a hello world program".into(),
            constraints: vec![],
            acceptance_criteria: vec!["Program outputs Hello, World!".into()],
            ontology: vec![],
            created_at: chrono::Utc::now(),
            generation: 0,
            parent_seed_id: None,
            cspace_hint: None,
            original_request: String::new(),
            output_schema: None,
        };

        let execution = oxios_ouroboros::ExecutionResult {
            output: "Hello, World!\n".into(),
            steps_completed: 1,
            success: true,
        };

        // First call: mechanical pass → skip LLM
        let result = engine
            .evaluate(&seed, &execution)
            .await
            .expect("eval failed");
        assert!(result.mechanical_pass, "Should mechanically pass");
        assert_eq!(result.score, 1.0, "Score should be 1.0 for mechanical pass");

        // Second call: cache hit
        let cached = engine
            .evaluate(&seed, &execution)
            .await
            .expect("cached eval failed");
        assert_eq!(
            cached.score, result.score,
            "Cached score should match original"
        );
    }
}
