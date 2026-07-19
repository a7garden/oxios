// Task store — SQLite-backed CRUD for tasks (RFC-043)
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::model::*;

/// SQLite-backed task store.
pub struct TaskStore {
    conn: Arc<Mutex<Connection>>,
}

impl TaskStore {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Result<Self> {
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.clone();
        // Block on init — called during startup
        let conn = conn.blocking_lock();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                identifier TEXT UNIQUE NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                instruction TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'backlog',
                priority INTEGER DEFAULT 0,
                sort_order REAL,
                parent_task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
                assignee_agent_id TEXT,
                created_by_agent_id TEXT,
                created_by_session_id TEXT,
                automation_mode TEXT,
                schedule_pattern TEXT,
                schedule_timezone TEXT,
                heartbeat_interval_secs INTEGER,
                max_executions INTEGER,
                execution_count INTEGER DEFAULT 0,
                verify_enabled INTEGER DEFAULT 0,
                verify_requirement TEXT,
                verify_max_iterations INTEGER DEFAULT 3,
                verify_verifier_agent_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                last_run_at TEXT,
                next_run_at TEXT,
                last_error TEXT,
                consecutive_failures INTEGER DEFAULT 0,
                context_json TEXT
            );

            CREATE TABLE IF NOT EXISTS task_dependencies (
                task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                depends_on TEXT NOT NULL,
                PRIMARY KEY (task_id, depends_on)
            );

            CREATE TABLE IF NOT EXISTS task_comments (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                content TEXT NOT NULL,
                author_agent_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT
            );

            CREATE TABLE IF NOT EXISTS task_runs (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                session_id TEXT,
                trigger TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'running',
                summary TEXT,
                result_content TEXT,
                started_at TEXT NOT NULL,
                completed_at TEXT,
                error TEXT,
                cost_usd REAL,
                tokens_used INTEGER
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_parent ON tasks(parent_task_id);
            CREATE INDEX IF NOT EXISTS idx_tasks_next_run ON tasks(next_run_at);
            CREATE INDEX IF NOT EXISTS idx_runs_task ON task_runs(task_id);
            "#,
        )?;
        Ok(())
    }

    pub async fn create_task(&self, params: CreateTaskParams) -> Result<Task> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();
        let identifier = params
            .identifier
            .unwrap_or_else(|| Task::slug_from_name(&params.name));

        conn.execute(
            r#"INSERT INTO tasks
               (id, identifier, name, description, instruction, status, priority,
                sort_order, parent_task_id, assignee_agent_id, created_at, updated_at,
                verify_enabled, execution_count, consecutive_failures)
               VALUES (?1, ?2, ?3, ?4, ?5, 'backlog', ?6, ?7, ?8, ?9, ?10, ?11, 0, 0, 0)"#,
            params![
                id,
                identifier,
                params.name,
                params.description,
                params.instruction,
                params.priority.unwrap_or(0),
                params.sort_order,
                params.parent_task_id,
                params.assignee_agent_id,
                now,
                now,
            ],
        )
        .context("insert task")?;

        self.get_task_by_id(&id).await
    }

    pub async fn get_task_by_id(&self, id: &str) -> Result<Task> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            r#"SELECT id, identifier, name, description, instruction, status, priority,
                      sort_order, parent_task_id, assignee_agent_id, created_by_agent_id,
                      created_by_session_id, automation_mode, schedule_pattern,
                      schedule_timezone, heartbeat_interval_secs, max_executions,
                      execution_count, verify_enabled, verify_requirement,
                      verify_max_iterations, verify_verifier_agent_id,
                      created_at, updated_at, started_at, completed_at,
                      last_run_at, next_run_at, last_error, consecutive_failures,
                      context_json
               FROM tasks WHERE id = ?1"#,
        )?;

        let task = stmt.query_row(params![id], map_task_row)?;
        Ok(task)
    }

    pub async fn list_tasks(&self, list_params: ListTasksParams) -> Result<Vec<Task>> {
        let conn = self.conn.lock().await;
        let limit = list_params.limit.unwrap_or(100).min(500);
        let offset = list_params.offset.unwrap_or(0);

        let mut sql = String::from(
            r#"SELECT id, identifier, name, description, instruction, status, priority,
                      sort_order, parent_task_id, assignee_agent_id, created_by_agent_id,
                      created_by_session_id, automation_mode, schedule_pattern,
                      schedule_timezone, heartbeat_interval_secs, max_executions,
                      execution_count, verify_enabled, verify_requirement,
                      verify_max_iterations, verify_verifier_agent_id,
                      created_at, updated_at, started_at, completed_at,
                      last_run_at, next_run_at, last_error, consecutive_failures,
                      context_json
               FROM tasks WHERE 1=1"#,
        );

        let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(limit), Box::new(offset)];

        if let Some(statuses) = &list_params.statuses {
            let placeholders: Vec<String> = statuses
                .iter()
                .enumerate()
                .map(|(i, _)| format!("?{}", param_values.len() + i + 1))
                .collect();
            sql.push_str(&format!(" AND status IN ({})", placeholders.join(",")));
            for s in statuses {
                param_values.push(Box::new(s.clone()));
            }
        }
        if let Some(ref assignee) = list_params.assignee_agent_id {
            sql.push_str(&format!(" AND assignee_agent_id = ?{}", param_values.len() + 1));
            param_values.push(Box::new(assignee.clone()));
        }
        if let Some(ref parent) = list_params.parent_task_id {
            sql.push_str(&format!(" AND parent_task_id = ?{}", param_values.len() + 1));
            param_values.push(Box::new(parent.clone()));
        }

        sql.push_str(&format!(" ORDER BY sort_order, created_at DESC LIMIT ?1 OFFSET ?2"));

        let param_refs: Vec<&dyn rusqlite::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let tasks = stmt
            .query_map(param_refs.as_slice(), map_task_row)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(tasks)
    }

    pub async fn delete_task(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])
            .context("delete task")?;
        Ok(())
    }

    pub async fn update_status(&self, id: &str, status: &TaskStatus) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        let completed = if *status == TaskStatus::Completed {
            Some(now.clone())
        } else {
            None
        };
        conn.execute(
            r#"UPDATE tasks SET status = ?1, updated_at = ?2, completed_at = COALESCE(?3, completed_at)
               WHERE id = ?4"#,
            params![status.to_string(), now, completed, id],
        )?;
        Ok(())
    }

    pub async fn list_due_tasks(&self) -> Result<Vec<Task>> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        let mut stmt = conn.prepare(
            r#"SELECT id, identifier, name, description, instruction, status, priority,
                      sort_order, parent_task_id, assignee_agent_id, created_by_agent_id,
                      created_by_session_id, automation_mode, schedule_pattern,
                      schedule_timezone, heartbeat_interval_secs, max_executions,
                      execution_count, verify_enabled, verify_requirement,
                      verify_max_iterations, verify_verifier_agent_id,
                      created_at, updated_at, started_at, completed_at,
                      last_run_at, next_run_at, last_error, consecutive_failures,
                      context_json
               FROM tasks
               WHERE automation_mode IS NOT NULL
                 AND status IN ('scheduled', 'running')
                 AND next_run_at IS NOT NULL
                 AND next_run_at <= ?1
               ORDER BY next_run_at"#,
        )?;
        let tasks = stmt
            .query_map(params![now], map_task_row)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(tasks)
    }

    pub async fn set_next_run(&self, id: &str, next_run: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE tasks SET next_run_at = ?1, updated_at = ?2 WHERE id = ?3",
            params![next_run, now, id],
        )?;
        Ok(())
    }
}

// ── Row mapper ──

fn map_task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let automation_mode_str: Option<String> = row.get(12)?;
    let automation_mode = automation_mode_str
        .as_deref()
        .and_then(|s| s.parse().ok());

    let status_str: String = row.get(5)?;
    let status = status_str.parse().unwrap_or(TaskStatus::Backlog);

    let context_json: Option<String> = row.get(30)?;
    let context: HashMap<String, serde_json::Value> = context_json
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    Ok(Task {
        id: row.get(0)?,
        identifier: row.get(1)?,
        name: row.get(2)?,
        description: row.get(3)?,
        instruction: row.get(4)?,
        status,
        priority: row.get(6)?,
        sort_order: row.get(7)?,
        parent_task_id: row.get(8)?,
        assignee_agent_id: row.get(9)?,
        created_by_agent_id: row.get(10)?,
        created_by_session_id: row.get(11)?,
        automation_mode,
        schedule_pattern: row.get(13)?,
        schedule_timezone: row.get(14)?,
        heartbeat_interval_secs: row.get(15)?,
        max_executions: row.get(16)?,
        execution_count: row.get(17)?,
        verify_enabled: row.get::<_, i64>(18)? != 0,
        verify_requirement: row.get(19)?,
        verify_max_iterations: row.get::<_, i64>(20)? as u32,
        verify_verifier_agent_id: row.get(21)?,
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
        started_at: row.get(24)?,
        completed_at: row.get(25)?,
        last_run_at: row.get(26)?,
        next_run_at: row.get(27)?,
        last_error: row.get(28)?,
        consecutive_failures: row.get::<_, i64>(29)? as u32,
        context,
        dependencies: Vec::new(),
    })
}
