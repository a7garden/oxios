//! TaskBank - predefined benchmark tasks
//!
//! Each task is a natural language command with expected outcomes
//! and an evaluation function.

use crate::{TaskCategory, TaskDefinition};
use crate::evaluator::{knowledge_evaluation, math_evaluation, memory_evaluation, web_search_evaluation, keyword_evaluation};

/// Get all predefined tasks
pub fn all_tasks() -> Vec<TaskDefinition> {
    vec![
        math_simple(),
        math_sqrt(),
        web_time_tokyo(),
        web_weather_seoul(),
        knowledge_capital(),
        knowledge_population(),
        session_memory(),
        code_explanation(),
        multi_turn_first(),
        web_ai_news(),
    ]
}

/// Get tasks by category
pub fn tasks_by_category(category: TaskCategory) -> Vec<TaskDefinition> {
    all_tasks()
        .into_iter()
        .filter(|t| t.category == category)
        .collect()
}

/// Get a single task by ID
pub fn task_by_id(id: &str) -> Option<TaskDefinition> {
    all_tasks().into_iter().find(|t| t.id == id)
}

// ---------------------------------------------------------------------------
// Math Tasks
// ---------------------------------------------------------------------------

pub fn math_simple() -> TaskDefinition {
    TaskDefinition {
        id: "math_simple",
        name: "Simple multiplication",
        category: TaskCategory::Math,
        command: "what is 17 * 23?",
        expected_outcomes: vec!["391"],
        evaluation_fn: |resp| math_evaluation(resp, "391"),
    }
}

pub fn math_sqrt() -> TaskDefinition {
    TaskDefinition {
        id: "math_sqrt",
        name: "Square root calculation",
        category: TaskCategory::Math,
        command: "what is the square root of 144?",
        expected_outcomes: vec!["12"],
        evaluation_fn: |resp| math_evaluation(resp, "12"),
    }
}

// ---------------------------------------------------------------------------
// Web Search Tasks
// ---------------------------------------------------------------------------

pub fn web_time_tokyo() -> TaskDefinition {
    TaskDefinition {
        id: "web_time_tokyo",
        name: "Current time in Tokyo",
        category: TaskCategory::WebSearch,
        command: "what is the current time in Tokyo?",
        expected_outcomes: vec!["Tokyo", "time"],
        evaluation_fn: |resp| web_search_evaluation(resp, &["Tokyo", "time"]),
    }
}

pub fn web_weather_seoul() -> TaskDefinition {
    TaskDefinition {
        id: "web_weather_seoul",
        name: "Weather in Seoul",
        category: TaskCategory::WebSearch,
        command: "what is the weather in Seoul today?",
        expected_outcomes: vec!["Seoul", "weather"],
        evaluation_fn: |resp| web_search_evaluation(resp, &["Seoul", "weather"]),
    }
}

pub fn web_ai_news() -> TaskDefinition {
    TaskDefinition {
        id: "web_ai_news",
        name: "AI news search",
        category: TaskCategory::WebSearch,
        command: "search the web for the latest news about AI agents",
        expected_outcomes: vec!["AI", "agent", "news"],
        evaluation_fn: |resp| web_search_evaluation(resp, &["AI", "agent"]),
    }
}

// ---------------------------------------------------------------------------
// Knowledge Tasks
// ---------------------------------------------------------------------------

pub fn knowledge_capital() -> TaskDefinition {
    TaskDefinition {
        id: "knowledge_capital",
        name: "Capital city question",
        category: TaskCategory::Knowledge,
        command: "what is the capital of Australia?",
        expected_outcomes: vec!["Canberra"],
        evaluation_fn: |resp| knowledge_evaluation(resp, "Canberra"),
    }
}

pub fn knowledge_population() -> TaskDefinition {
    TaskDefinition {
        id: "knowledge_population",
        name: "Population question",
        category: TaskCategory::Knowledge,
        command: "what is the population of Tokyo?",
        expected_outcomes: vec!["Tokyo", "population"],
        evaluation_fn: |resp| keyword_evaluation(resp, &["Tokyo", "population", "million"]),
    }
}

// ---------------------------------------------------------------------------
// Memory Tasks
// ---------------------------------------------------------------------------

pub fn session_memory() -> TaskDefinition {
    TaskDefinition {
        id: "session_memory",
        name: "Remember user preference",
        category: TaskCategory::Memory,
        command: "remember this: my favorite color is blue",
        expected_outcomes: vec!["remember", "blue"],
        evaluation_fn: |resp| memory_evaluation(resp, "blue"),
    }
}

// ---------------------------------------------------------------------------
// Coding Tasks
// ---------------------------------------------------------------------------

pub fn code_explanation() -> TaskDefinition {
    TaskDefinition {
        id: "code_explanation",
        name: "Explain code",
        category: TaskCategory::Coding,
        command: "explain what the following code does: function add(a, b) { return a + b; }",
        expected_outcomes: vec!["add", "return", "function"],
        evaluation_fn: |resp| keyword_evaluation(resp, &["add", "function", "return", "sum"]),
    }
}

// ---------------------------------------------------------------------------
// Multi-turn Tasks
// ---------------------------------------------------------------------------

pub fn multi_turn_first() -> TaskDefinition {
    TaskDefinition {
        id: "multi_turn_first",
        name: "Multi-turn conversation start",
        category: TaskCategory::MultiTurn,
        command: "my name is John",
        expected_outcomes: vec!["John", "hello", "name"],
        evaluation_fn: |resp| keyword_evaluation(resp, &["John", "hello", "name"]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_tasks() {
        let tasks = all_tasks();
        assert_eq!(tasks.len(), 10);
    }

    #[test]
    fn test_task_by_id() {
        let task = task_by_id("math_simple");
        assert!(task.is_some());
        assert_eq!(task.unwrap().id, "math_simple");
    }

    #[test]
    fn test_tasks_by_category() {
        let math_tasks = tasks_by_category(TaskCategory::Math);
        assert!(!math_tasks.is_empty());
    }
}