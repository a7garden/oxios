//! Seed and ambiguity score unit tests — no LLM required.

use oxios_ouroboros::seed::{AmbiguityScore, Entity, Seed};

#[test]
fn test_seed_new_generates_id_and_defaults() {
    let seed = Seed::new("Build a REST API");
    assert!(!seed.goal.is_empty());
    assert_eq!(seed.goal, "Build a REST API");
    assert_eq!(seed.generation, 0);
    assert!(seed.parent_seed_id.is_none());
    assert!(seed.constraints.is_empty());
    assert!(seed.acceptance_criteria.is_empty());
    assert!(seed.ontology.is_empty());
}

#[test]
fn test_seed_evolved_from_increments_generation() {
    let parent = Seed::new("Build a REST API");
    let child = Seed::evolved_from(&parent);
    assert_eq!(child.generation, 1);
    assert_eq!(child.parent_seed_id, Some(parent.id));
}

#[test]
fn test_seed_evolved_from_preserves_goal() {
    let parent = Seed::new("Build a REST API");
    let child = Seed::evolved_from(&parent);
    assert_eq!(child.goal, parent.goal);
    assert_eq!(child.constraints, parent.constraints);
    assert_eq!(child.acceptance_criteria, parent.acceptance_criteria);
}

#[test]
fn test_seed_lineage_chain() {
    let gen0 = Seed::new("Goal");
    let gen1 = Seed::evolved_from(&gen0);
    let gen2 = Seed::evolved_from(&gen1);
    assert_eq!(gen0.generation, 0);
    assert_eq!(gen1.generation, 1);
    assert_eq!(gen2.generation, 2);
    assert_eq!(gen2.parent_seed_id, Some(gen1.id));
    assert_eq!(gen1.parent_seed_id, Some(gen0.id));
}

#[test]
fn test_seed_is_immutable() {
    let original = Seed::new("Goal");
    let _evolved = Seed::evolved_from(&original);
    // Original must be unchanged
    assert_eq!(original.generation, 0);
    assert!(original.parent_seed_id.is_none());
}

#[test]
fn test_seed_with_fields() {
    let mut seed = Seed::new("Deploy service");
    seed.constraints.push("Must use HTTPS".into());
    seed.acceptance_criteria.push("Returns 200 OK".into());
    seed.ontology.push(Entity {
        name: "API Gateway".into(),
        entity_type: "service".into(),
        description: "Entry point for external traffic".into(),
    });
    assert_eq!(seed.constraints.len(), 1);
    assert_eq!(seed.acceptance_criteria.len(), 1);
    assert_eq!(seed.ontology.len(), 1);
}

#[test]
fn test_ambiguity_score_perfect_clarity() {
    let score = AmbiguityScore::new(1.0, 1.0, 1.0);
    assert!(score.ambiguity() < 0.01);
    assert!(score.is_ready());
}

#[test]
fn test_ambiguity_score_max_ambiguity() {
    let score = AmbiguityScore::new(0.0, 0.0, 0.0);
    assert!((score.ambiguity() - 1.0).abs() < 0.01);
    assert!(!score.is_ready());
}

#[test]
fn test_ambiguity_score_weighted_calculation() {
    // goal 40%, constraints 30%, criteria 30%
    let score = AmbiguityScore::new(0.5, 1.0, 1.0);
    // clarity = 0.5*0.4 + 1.0*0.3 + 1.0*0.3 = 0.2 + 0.3 + 0.3 = 0.8
    // ambiguity = 1.0 - 0.8 = 0.2
    assert!((score.ambiguity() - 0.2).abs() < 0.001);
    assert!(score.is_ready()); // exactly at threshold
}

#[test]
fn test_ambiguity_score_clamped_values() {
    let over = AmbiguityScore::new(1.5, -0.3, 2.0);
    assert_eq!(over.goal_clarity, 1.0);
    assert_eq!(over.constraint_clarity, 0.0);
    assert_eq!(over.success_criteria, 1.0);
}

#[test]
fn test_ambiguity_threshold_boundary() {
    let just_above = AmbiguityScore::new(0.49, 1.0, 1.0);
    // clarity = 0.49*0.4 + 1.0*0.3 + 1.0*0.3 = 0.196 + 0.3 + 0.3 = 0.796
    // ambiguity = 0.204 > 0.2 → not ready
    assert!(!just_above.is_ready());

    let just_below = AmbiguityScore::new(0.51, 1.0, 1.0);
    // clarity = 0.51*0.4 + 0.3 + 0.3 = 0.204 + 0.6 = 0.804
    // ambiguity = 0.196 < 0.2 → ready
    assert!(just_below.is_ready());
}

#[test]
fn test_default_ambiguity_is_max() {
    let score = AmbiguityScore::default();
    assert_eq!(score.goal_clarity, 0.0);
    assert_eq!(score.constraint_clarity, 0.0);
    assert_eq!(score.success_criteria, 0.0);
    assert!(!score.is_ready());
}
