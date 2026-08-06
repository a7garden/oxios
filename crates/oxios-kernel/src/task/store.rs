// Task store — SQLite-backed CRUD for tasks (RFC-043)
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::model::*;

/// SQLite-backed task store.
pub struct TaskStore {
    conn: Arc<Mutex<Connection>>,
}

impl TaskStore {
    /// Create a TaskStore from a raw connection. Schema is initialized
    /// on the connection *before* it is wrapped in the async mutex, so
    /// this constructor is safe to call from inside a Tokio runtime —
    /// no `blocking_lock` is involved.
    pub fn new(conn: Connection) -> Result<Self> {
        init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create a TaskStore from a database file path.
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open task database: {path}"))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Self::new(conn)
    }

    /// Create an in-memory TaskStore (for tests).
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::new(conn)
    }

    pub async fn create_task(&self, params: CreateTaskParams) -> Result<Task> {
        let id = uuid::Uuid::new_v4().to_string();
        {
            let conn = self.conn.lock().await;
            let now = Utc::now().to_rfc3339();
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
        }
        // Lock released — safe to call another `&self` method.
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

        let mut param_values: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(limit), Box::new(offset)];

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
            sql.push_str(&format!(
                " AND assignee_agent_id = ?{}",
                param_values.len() + 1
            ));
            param_values.push(Box::new(assignee.clone()));
        }
        if let Some(ref parent) = list_params.parent_task_id {
            sql.push_str(&format!(
                " AND parent_task_id = ?{}",
                param_values.len() + 1
            ));
            param_values.push(Box::new(parent.clone()));
        }

        sql.push_str(" ORDER BY sort_order, created_at DESC LIMIT ?1 OFFSET ?2");

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
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
                 AND status = 'scheduled'
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
    // ── Automation / scheduling ───────────────────────────────────────

    /// Persist automation/schedule fields and set status + `next_run_at`.
    ///
    /// - Schedule mode → `next_run_at` = next cron fire after now.
    /// - Heartbeat mode → `next_run_at` = now + interval.
    /// - No automation (mode None) → clears scheduling: status `backlog`,
    ///   `next_run_at` NULL.
    pub async fn set_automation(&self, id: &str, params: SetScheduleParams) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now();
        let now_rfc = now.to_rfc3339();

        let next_run = match &params.automation_mode {
            Some(TaskAutomationMode::Schedule) => params
                .schedule_pattern
                .as_deref()
                .and_then(|p| cron_next(p, &now).ok()),
            Some(TaskAutomationMode::Heartbeat) => params
                .heartbeat_interval_secs
                .map(|secs| (now + chrono::Duration::seconds(secs as i64)).to_rfc3339()),
            None => None,
        };

        let mode_str = params.automation_mode.as_ref().map(|m| m.to_string());
        let status = if params.automation_mode.is_some() {
            "scheduled"
        } else {
            "backlog"
        };

        conn.execute(
            r#"UPDATE tasks SET
                 automation_mode = ?1, schedule_pattern = ?2, schedule_timezone = ?3,
                 heartbeat_interval_secs = ?4, max_executions = ?5,
                 status = ?6, next_run_at = ?7, updated_at = ?8
               WHERE id = ?9"#,
            params![
                mode_str,
                params.schedule_pattern,
                params.schedule_timezone,
                params.heartbeat_interval_secs.map(|v| v as i64),
                params.max_executions,
                status,
                next_run,
                now_rfc,
                id,
            ],
        )?;
        Ok(())
    }

    // ── Execution lifecycle (backed by task_runs) ─────────────────────

    /// Mark a task as Running: set `started_at`/`last_run_at` and insert a
    /// `task_runs` row tagged with `trigger`. Returns the new run id so the
    /// caller can finalize it via [`Self::mark_finished`].
    pub async fn mark_running(&self, id: &str, trigger: TaskRunTrigger) -> Result<String> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE tasks SET status = 'running', started_at = ?1, last_run_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        let run_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            r#"INSERT INTO task_runs (id, task_id, trigger, status, started_at)
               VALUES (?1, ?2, ?3, 'running', ?4)"#,
            params![run_id, id, trigger.to_string(), now],
        )?;
        Ok(run_id)
    }

    /// Finalize a run: update the `task_runs` row and the task's terminal
    /// state (status, execution_count, consecutive_failures, timestamps).
    /// If the task still has active automation and hasn't hit `max_executions`,
    /// recompute `next_run_at` and flip status back to `scheduled`; otherwise
    /// leave it terminal (`completed`/`failed`).
    pub async fn mark_finished(
        &self,
        id: &str,
        run_id: &str,
        success: bool,
        summary: String,
        error: Option<String>,
    ) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now();
        let now_rfc = now.to_rfc3339();
        let run_status = if success { "completed" } else { "failed" };

        // 1. task_runs row: terminal status + payload + completed_at.
        conn.execute(
            r#"UPDATE task_runs SET status = ?1, summary = ?2, result_content = ?3,
               error = ?4, completed_at = ?5 WHERE id = ?6"#,
            params![run_status, &summary, &summary, &error, &now_rfc, run_id],
        )?;

        // 2. Read automation config + counts to decide terminal vs. reschedule.
        let (mode, pattern, hb_secs, exec_count, max_exec, consec_failures): (
            Option<String>,
            Option<String>,
            Option<i64>,
            i64,
            Option<i64>,
            i64,
        ) = conn.query_row(
            "SELECT automation_mode, schedule_pattern, heartbeat_interval_secs, \
             execution_count, max_executions, consecutive_failures FROM tasks WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )?;

        let new_count = exec_count + 1;
        let exhausted = max_exec.is_some_and(|m| new_count >= m);

        // Recompute next_run only on success and when automation is active
        // and not exhausted. On failure we still reschedule (transient errors
        // shouldn't permanently disable a scheduled task) but cap via
        // consecutive_failures.
        let reschedule = mode.is_some() && !exhausted;
        let next_run = if reschedule {
            match mode.as_deref() {
                Some("schedule") => pattern.as_deref().and_then(|p| cron_next(p, &now).ok()),
                Some("heartbeat") => {
                    hb_secs.map(|s| (now + chrono::Duration::seconds(s)).to_rfc3339())
                }
                _ => None,
            }
        } else {
            None
        };

        let terminal_status = if reschedule {
            "scheduled"
        } else if success {
            "completed"
        } else {
            "failed"
        };
        // Reset consecutive_failures to 0 on success; increment on failure.
        let new_consec = if success { 0 } else { consec_failures + 1 };
        conn.execute(
            r#"UPDATE tasks SET
                 status = ?1, execution_count = ?2,
                 completed_at = COALESCE(?3, completed_at),
                 consecutive_failures = ?4,
                 last_error = ?5, next_run_at = ?6, updated_at = ?7
               WHERE id = ?8"#,
            params![
                terminal_status,
                new_count,
                if !reschedule { Some(&now_rfc) } else { None },
                new_consec,
                error,
                next_run,
                now_rfc,
                id,
            ],
        )?;
        Ok(())
    }

    /// Latest run for a task (for the UI's "last result" display).
    pub async fn latest_run(&self, task_id: &str) -> Result<Option<TaskRun>> {
        let conn = self.conn.lock().await;
        Ok(conn
            .query_row(
                "SELECT id, task_id, session_id, trigger, status, summary, result_content, \
                 started_at, completed_at, error, cost_usd, tokens_used FROM task_runs \
                 WHERE task_id = ?1 ORDER BY started_at DESC LIMIT 1",
                params![task_id],
                map_run_row,
            )
            .optional()?)
    }

    /// Run history for a task (newest first).
    pub async fn list_runs(&self, task_id: &str) -> Result<Vec<TaskRun>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, session_id, trigger, status, summary, result_content, \
             started_at, completed_at, error, cost_usd, tokens_used FROM task_runs \
             WHERE task_id = ?1 ORDER BY started_at DESC LIMIT 50",
        )?;
        let runs = stmt
            .query_map(params![task_id], map_run_row)?
            .filter_map(|r| r.ok())
            .collect();
        Ok(runs)
    }

    /// Boot-time recovery: reset tasks stranded at `running` by a prior
    /// process crash (the Task model persists status to SQLite, unlike the
    /// CronScheduler's in-memory `running_jobs` set). Since `list_due_tasks`
    /// excludes `running`, stranded tasks would otherwise never be retried.
    ///
    /// - Orphaned `task_runs` rows still `running` → marked `failed`.
    /// - Stranded tasks → `scheduled` (if automation is set) or `backlog`.
    pub async fn recover_stranded(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        let runs = conn.execute(
            "UPDATE task_runs SET status = 'failed', \
             error = 'Interrupted by process restart', completed_at = ?1 \
             WHERE status = 'running'",
            params![now],
        )?;
        let tasks = conn.execute(
            "UPDATE tasks SET status = CASE WHEN automation_mode IS NOT NULL \
             THEN 'scheduled' ELSE 'backlog' END, updated_at = ?1 \
             WHERE status = 'running'",
            params![now],
        )?;
        if runs > 0 || tasks > 0 {
            tracing::info!(runs, tasks, "Recovered stranded tasks/runs after restart");
        }
        Ok(())
    }
}
fn init_schema(conn: &Connection) -> Result<()> {
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

// ── Row mapper ──

fn map_task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let automation_mode_str: Option<String> = row.get(12)?;
    let automation_mode = automation_mode_str.as_deref().and_then(|s| s.parse().ok());

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
        heartbeat_interval_secs: row.get::<_, Option<i64>>(15)?.map(|v| v as u64),
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

fn map_run_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRun> {
    let trigger_str: String = row.get(3)?;
    let trigger = trigger_str.parse().unwrap_or(TaskRunTrigger::Manual);
    Ok(TaskRun {
        id: row.get(0)?,
        task_id: row.get(1)?,
        session_id: row.get(2)?,
        trigger,
        status: row.get(4)?,
        summary: row.get(5)?,
        result_content: row.get(6)?,
        started_at: row.get(7)?,
        completed_at: row.get(8)?,
        error: row.get(9)?,
        cost_usd: row.get(10)?,
        tokens_used: row.get::<_, Option<i64>>(11)?.map(|v| v as u64),
    })
}

/// Compute the next cron fire time after `after` as an RFC3339 string.
/// Normalizes 5-field (Linux cron) expressions by prepending a seconds field,
/// matching `CronScheduler::normalize_expr`.
fn cron_next(pattern: &str, after: &DateTime<Utc>) -> Result<String> {
    let normalized = {
        let fields: Vec<&str> = pattern.split_whitespace().collect();
        if fields.len() == 5 {
            format!("0 {pattern}")
        } else {
            pattern.to_string()
        }
    };
    let schedule = cron::Schedule::from_str(&normalized)
        .map_err(|e| anyhow::anyhow!("Invalid cron expression '{pattern}': {e}"))?;
    let next = schedule
        .after(after)
        .next()
        .ok_or_else(|| anyhow::anyhow!("No future fire time for cron '{pattern}'"))?;
    Ok(next.to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_params(name: &str) -> CreateTaskParams {
        CreateTaskParams {
            name: name.to_string(),
            instruction: format!("do {name}"),
            identifier: None,
            description: None,
            priority: None,
            parent_task_id: None,
            assignee_agent_id: None,
            sort_order: None,
        }
    }

    // Regression: `TaskStore::open` / `in_memory` must be safe to call from
    // inside a Tokio runtime. The production web surface constructs the
    // store on the runtime (`src/api/plugin.rs`); an earlier version used
    // `blocking_lock()` during schema init and panicked at startup with
    // "Cannot block the current thread from within a runtime".
    #[tokio::test]
    async fn in_memory_store_construction_does_not_panic_on_runtime() {
        let store = TaskStore::in_memory().expect("in-memory store builds");
        // Sanity: schema is usable.
        let task = store
            .create_task(sample_params("regression"))
            .await
            .expect("create works");
        assert_eq!(task.name, "regression");
    }

    #[tokio::test]
    async fn open_from_file_path_works_on_runtime() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("tasks.db");
        let path_str = path.to_str().expect("utf8 path");
        let store = TaskStore::open(path_str).expect("open builds");
        let created = store
            .create_task(sample_params("from-disk"))
            .await
            .expect("create");
        // Re-open the same file — schema init must be idempotent and the
        // row must survive reopen.
        drop(store);
        let reopened = TaskStore::open(path_str).expect("reopen builds");
        let fetched = reopened
            .get_task_by_id(&created.id)
            .await
            .expect("get_by_id");
        assert_eq!(fetched.name, "from-disk");
    }

    #[tokio::test]
    async fn create_list_update_delete_roundtrip() {
        let store = TaskStore::in_memory().expect("in-memory store builds");
        let t1 = store
            .create_task(sample_params("alpha"))
            .await
            .expect("create alpha");
        let _t2 = store
            .create_task(sample_params("beta"))
            .await
            .expect("create beta");

        let listed = store
            .list_tasks(ListTasksParams::default())
            .await
            .expect("list");
        assert_eq!(listed.len(), 2);

        store
            .update_status(&t1.id, &TaskStatus::Completed)
            .await
            .expect("update");
        let fetched = store.get_task_by_id(&t1.id).await.expect("get_by_id");
        assert_eq!(fetched.status, TaskStatus::Completed);
        assert!(fetched.completed_at.is_some());

        store.delete_task(&t1.id).await.expect("delete");
        let after = store
            .list_tasks(ListTasksParams::default())
            .await
            .expect("list after delete");
        assert_eq!(after.len(), 1);
    }
    // ── Scheduling ──

    #[tokio::test]
    async fn set_automation_schedule_computes_next_run_and_status() {
        let store = TaskStore::in_memory().unwrap();
        let t = store.create_task(sample_params("sched")).await.unwrap();
        store
            .set_automation(
                &t.id,
                SetScheduleParams {
                    automation_mode: Some(TaskAutomationMode::Schedule),
                    schedule_pattern: Some("0 9 * * *".into()),
                    schedule_timezone: None,
                    heartbeat_interval_secs: None,
                    max_executions: None,
                },
            )
            .await
            .unwrap();
        let fetched = store.get_task_by_id(&t.id).await.unwrap();
        assert_eq!(fetched.status, TaskStatus::Scheduled);
        assert_eq!(fetched.schedule_pattern.as_deref(), Some("0 9 * * *"));
        assert!(
            fetched.next_run_at.is_some(),
            "next_run_at must be computed"
        );
    }

    #[tokio::test]
    async fn set_automation_heartbeat_computes_next_run() {
        let store = TaskStore::in_memory().unwrap();
        let t = store.create_task(sample_params("hb")).await.unwrap();
        store
            .set_automation(
                &t.id,
                SetScheduleParams {
                    automation_mode: Some(TaskAutomationMode::Heartbeat),
                    schedule_pattern: None,
                    schedule_timezone: None,
                    heartbeat_interval_secs: Some(600),
                    max_executions: None,
                },
            )
            .await
            .unwrap();
        let fetched = store.get_task_by_id(&t.id).await.unwrap();
        assert_eq!(fetched.status, TaskStatus::Scheduled);
        assert_eq!(fetched.heartbeat_interval_secs, Some(600));
        assert!(fetched.next_run_at.is_some());
    }

    #[tokio::test]
    async fn set_automation_none_clears_scheduling() {
        let store = TaskStore::in_memory().unwrap();
        let t = store.create_task(sample_params("clear")).await.unwrap();
        // First schedule, then clear.
        store
            .set_automation(
                &t.id,
                SetScheduleParams {
                    automation_mode: Some(TaskAutomationMode::Heartbeat),
                    schedule_pattern: None,
                    schedule_timezone: None,
                    heartbeat_interval_secs: Some(60),
                    max_executions: None,
                },
            )
            .await
            .unwrap();
        store
            .set_automation(
                &t.id,
                SetScheduleParams {
                    automation_mode: None,
                    schedule_pattern: None,
                    schedule_timezone: None,
                    heartbeat_interval_secs: None,
                    max_executions: None,
                },
            )
            .await
            .unwrap();
        let fetched = store.get_task_by_id(&t.id).await.unwrap();
        assert_eq!(fetched.status, TaskStatus::Backlog);
        assert!(fetched.next_run_at.is_none());
        assert!(fetched.automation_mode.is_none());
    }

    // ── Run lifecycle ──

    #[tokio::test]
    async fn mark_running_then_finished_success_reschedules_and_counts() {
        let store = TaskStore::in_memory().unwrap();
        let t = store.create_task(sample_params("lifecycle")).await.unwrap();
        // Give it a heartbeat schedule so success reschedules.
        store
            .set_automation(
                &t.id,
                SetScheduleParams {
                    automation_mode: Some(TaskAutomationMode::Heartbeat),
                    schedule_pattern: None,
                    schedule_timezone: None,
                    heartbeat_interval_secs: Some(300),
                    max_executions: None,
                },
            )
            .await
            .unwrap();

        let run_id = store
            .mark_running(&t.id, TaskRunTrigger::Manual)
            .await
            .unwrap();
        let mid = store.get_task_by_id(&t.id).await.unwrap();
        assert_eq!(mid.status, TaskStatus::Running);
        assert!(mid.started_at.is_some());

        store
            .mark_finished(&t.id, &run_id, true, "ok".into(), None)
            .await
            .unwrap();
        let done = store.get_task_by_id(&t.id).await.unwrap();
        assert_eq!(done.status, TaskStatus::Scheduled, "success reschedules");
        assert_eq!(done.execution_count, 1);
        assert_eq!(done.consecutive_failures, 0, "success resets failures");
        assert!(done.next_run_at.is_some(), "next_run recomputed");

        // Run history recorded.
        let runs = store.list_runs(&t.id).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "completed");
        assert_eq!(runs[0].summary.as_deref(), Some("ok"));
    }

    #[tokio::test]
    async fn consecutive_failures_reset_on_success() {
        let store = TaskStore::in_memory().unwrap();
        let t = store.create_task(sample_params("flap")).await.unwrap();
        store
            .set_automation(
                &t.id,
                SetScheduleParams {
                    automation_mode: Some(TaskAutomationMode::Heartbeat),
                    schedule_pattern: None,
                    schedule_timezone: None,
                    heartbeat_interval_secs: Some(60),
                    max_executions: None,
                },
            )
            .await
            .unwrap();

        // Two failures.
        for _ in 0..2 {
            let rid = store
                .mark_running(&t.id, TaskRunTrigger::Heartbeat)
                .await
                .unwrap();
            store
                .mark_finished(&t.id, &rid, false, String::new(), Some("boom".into()))
                .await
                .unwrap();
        }
        let after_fails = store.get_task_by_id(&t.id).await.unwrap();
        assert_eq!(after_fails.consecutive_failures, 2);

        // Then a success — consecutive_failures must reset to 0.
        let rid = store
            .mark_running(&t.id, TaskRunTrigger::Heartbeat)
            .await
            .unwrap();
        store
            .mark_finished(&t.id, &rid, true, "recovered".into(), None)
            .await
            .unwrap();
        let after_ok = store.get_task_by_id(&t.id).await.unwrap();
        assert_eq!(after_ok.consecutive_failures, 0, "reset on success");
        assert_eq!(after_ok.execution_count, 3);
    }

    #[tokio::test]
    async fn max_executions_exhausts_to_completed() {
        let store = TaskStore::in_memory().unwrap();
        let t = store.create_task(sample_params("max")).await.unwrap();
        store
            .set_automation(
                &t.id,
                SetScheduleParams {
                    automation_mode: Some(TaskAutomationMode::Heartbeat),
                    schedule_pattern: None,
                    schedule_timezone: None,
                    heartbeat_interval_secs: Some(60),
                    max_executions: Some(2),
                },
            )
            .await
            .unwrap();
        for _ in 0..2 {
            let rid = store
                .mark_running(&t.id, TaskRunTrigger::Heartbeat)
                .await
                .unwrap();
            store
                .mark_finished(&t.id, &rid, true, "ok".into(), None)
                .await
                .unwrap();
        }
        let done = store.get_task_by_id(&t.id).await.unwrap();
        assert_eq!(done.status, TaskStatus::Completed, "exhausted → completed");
        assert_eq!(done.execution_count, 2);
        assert!(done.next_run_at.is_none(), "no further reschedule");
    }

    // ── Stranded recovery ──

    #[tokio::test]
    async fn recover_stranded_resets_running_tasks() {
        let store = TaskStore::in_memory().unwrap();
        // Task WITH automation → should recover to 'scheduled'.
        let t_auto = store.create_task(sample_params("auto")).await.unwrap();
        store
            .set_automation(
                &t_auto.id,
                SetScheduleParams {
                    automation_mode: Some(TaskAutomationMode::Heartbeat),
                    schedule_pattern: None,
                    schedule_timezone: None,
                    heartbeat_interval_secs: Some(60),
                    max_executions: None,
                },
            )
            .await
            .unwrap();
        // Task WITHOUT automation → should recover to 'backlog'.
        let t_plain = store.create_task(sample_params("plain")).await.unwrap();

        // Simulate a crash mid-run: mark both running.
        store
            .mark_running(&t_auto.id, TaskRunTrigger::Manual)
            .await
            .unwrap();
        store
            .mark_running(&t_plain.id, TaskRunTrigger::Manual)
            .await
            .unwrap();

        store.recover_stranded().await.unwrap();

        let auto = store.get_task_by_id(&t_auto.id).await.unwrap();
        assert_eq!(
            auto.status,
            TaskStatus::Scheduled,
            "automated task rescheduled"
        );
        let plain = store.get_task_by_id(&t_plain.id).await.unwrap();
        assert_eq!(
            plain.status,
            TaskStatus::Backlog,
            "plain task back to backlog"
        );

        // Orphaned task_runs rows closed as failed.
        let runs = store.list_runs(&t_auto.id).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "failed");
        assert!(runs[0].error.is_some());
    }
}
