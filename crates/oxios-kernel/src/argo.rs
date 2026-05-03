//! Argo Workflows integration for agent task execution.
//!
//! This module provides integration with Argo Workflows for executing
//! long-running agent tasks as Kubernetes workflows.
//!
//! ## Authentication
//!
//! Authentication is handled via Bearer token from:
//! 1. `ARGO_TOKEN` environment variable (preferred)
//! 2. Service account token mounted in the pod (default path: `/var/run/secrets/kubernetes.io/serviceaccount/token`)
//!
//! ## API Endpoints
//!
//! Uses Argo Workflows API v1:
//! - `POST /api/v1/workflows/{namespace}/submit` - Submit workflow
//! - `GET /api/v1/workflows/{namespace}/{name}` - Get workflow status
//! - `GET /api/v1/workflows/{namespace}` - List workflows
//! - `DELETE /api/v1/workflows/{namespace}/{name}` - Delete workflow

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Workflow status enum representing all possible states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowPhase {
    /// Workflow has been created but not yet started.
    Pending,
    /// Workflow is currently running.
    Running,
    /// Workflow completed successfully.
    Succeeded,
    /// Workflow completed with failures.
    Failed,
    /// Workflow was errored.
    Error,
    /// Workflow was aborted/cancelled.
    Archived,
}

impl Default for WorkflowPhase {
    fn default() -> Self {
        WorkflowPhase::Pending
    }
}

/// Status information for a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStatus {
    /// Workflow name.
    pub name: String,
    /// Current phase/status.
    pub phase: WorkflowPhase,
    /// Namespace where workflow is running.
    pub namespace: String,
    /// When the workflow started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    /// When the workflow finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    /// Duration in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<i64>,
    /// Workflow message (errors, etc).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Summary of a workflow for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSummary {
    /// Workflow name.
    pub name: String,
    /// Current phase/status.
    pub phase: WorkflowPhase,
    /// Namespace.
    pub namespace: String,
    /// When the workflow started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    /// When the workflow finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
}

/// Configuration for Argo Workflows connection.
#[derive(Debug, Clone)]
pub struct ArgoConfig {
    /// URL of the Argo server API (e.g., "https://argo.example.com").
    pub api_server: String,
    /// Kubernetes namespace for workflows.
    pub namespace: String,
    /// Service account to use for workflow execution.
    pub service_account: String,
    /// Default workflow template name.
    pub workflow_template: Option<String>,
}

impl Default for ArgoConfig {
    fn default() -> Self {
        Self {
            api_server: env::var("ARGO_SERVER").unwrap_or_else(|_| "http://localhost:2746".to_string()),
            namespace: env::var("ARGO_NAMESPACE").unwrap_or_else(|_| "default".to_string()),
            service_account: env::var("ARGO_SERVICE_ACCOUNT").unwrap_or_else(|_| "argo".to_string()),
            workflow_template: env::var("ARGO_WORKFLOW_TEMPLATE").ok(),
        }
    }
}

/// Argo Workflows client for managing workflow execution.
#[derive(Debug, Clone)]
pub struct ArgoWorkflow {
    config: ArgoConfig,
    client: Client,
    token: Arc<RwLock<Option<String>>>,
}

impl ArgoWorkflow {
    /// Create a new Argo Workflows client from configuration.
    pub fn new(config: ArgoConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            config,
            client,
            token: Arc::new(RwLock::new(None)),
        })
    }

    /// Create a new client with default configuration.
    pub fn default_client() -> Result<Self> {
        Self::new(ArgoConfig::default())
    }

    /// Initialize authentication token.
    /// Checks ARGO_TOKEN env var first, then falls back to service account token file.
    pub async fn init_auth(&self) -> Result<()> {
        // First try ARGO_TOKEN environment variable
        if let Ok(token) = env::var("ARGO_TOKEN") {
            let mut auth_token = self.token.write().await;
            *auth_token = Some(token);
            info!("Using ARGO_TOKEN for authentication");
            return Ok(());
        }

        // Fall back to service account token
        let sa_token_path = "/var/run/secrets/kubernetes.io/serviceaccount/token";
        if let Ok(token) = fs::read_to_string(sa_token_path) {
            let mut auth_token = self.token.write().await;
            *auth_token = Some(token.trim().to_string());
            info!("Using service account token for authentication");
            return Ok(());
        }

        warn!("No Argo authentication token found. Set ARGO_TOKEN or run inside a pod with service account.");
        Ok(())
    }

    /// Get the current auth token.
    async fn get_token(&self) -> Option<String> {
        self.token.read().await.clone()
    }

    /// Build authorization header value.
    fn auth_header(token: &str) -> String {
        format!("Bearer {}", token)
    }

    /// Build the base URL for API calls.
    fn base_url(&self) -> String {
        format!(
            "{}/api/v1/workflows/{}",
            self.config.api_server.trim_end_matches('/'),
            self.config.namespace
        )
    }

    /// Submit a workflow from YAML definition.
    ///
    /// Returns the workflow name on success.
    pub async fn submit_workflow(&self, workflow_yaml: &str) -> Result<String> {
        let token = self.get_token().await
            .context("No authentication token. Call init_auth() first.")?;

        let url = format!("{}/submit", self.base_url());
        debug!("Submitting workflow to: {}", url);

        let response = self.client
            .post(&url)
            .header("Authorization", Self::auth_header(&token))
            .header("Content-Type", "application/x-yaml")
            .body(workflow_yaml.to_string())
            .send()
            .await
            .context("Failed to submit workflow")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Workflow submission failed: {} - {}", status, body);
            anyhow::bail!("Workflow submission failed: {} - {}", status, body);
        }

        #[derive(Deserialize)]
        struct SubmitResponse {
            #[serde(rename = "metadata")]
            metadata: WorkflowMetadata,
        }

        #[derive(Deserialize)]
        struct WorkflowMetadata {
            name: String,
        }

        let result: SubmitResponse = response.json().await
            .context("Failed to parse submit response")?;

        let workflow_name = result.metadata.name;
        info!("Workflow submitted successfully: {}", workflow_name);

        Ok(workflow_name)
    }

    /// Get the status of a workflow by name.
    pub async fn get_workflow_status(&self, name: &str) -> Result<WorkflowStatus> {
        let token = self.get_token().await
            .context("No authentication token. Call init_auth() first.")?;

        let url = format!("{}/{}", self.base_url(), name);
        debug!("Getting workflow status: {}", url);

        let response = self.client
            .get(&url)
            .header("Authorization", Self::auth_header(&token))
            .send()
            .await
            .context("Failed to get workflow status")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Get workflow failed: {} - {}", status, body);
            anyhow::bail!("Get workflow failed: {} - {}", status, body);
        }

        #[derive(Deserialize)]
        struct WorkflowResponse {
            #[serde(rename = "metadata")]
            metadata: WorkflowMetadata,
            #[serde(rename = "status")]
            status: InternalStatus,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct WorkflowMetadata {
            name: String,
            namespace: String,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct InternalStatus {
            phase: String,
            started_at: Option<DateTime<Utc>>,
            finished_at: Option<DateTime<Utc>>,
            message: Option<String>,
        }

        let wf: WorkflowResponse = response.json().await
            .context("Failed to parse workflow status response")?;

        let duration_seconds = if let (Some(start), Some(end)) = (wf.status.started_at, wf.status.finished_at) {
            Some((end - start).num_seconds())
        } else {
            None
        };

        let phase = parse_phase(&wf.status.phase);

        Ok(WorkflowStatus {
            name: wf.metadata.name,
            phase,
            namespace: wf.metadata.namespace,
            started_at: wf.status.started_at,
            finished_at: wf.status.finished_at,
            duration_seconds,
            message: wf.status.message,
        })
    }

    /// List recent workflows.
    pub async fn list_workflows(&self) -> Result<Vec<WorkflowSummary>> {
        let token = self.get_token().await
            .context("No authentication token. Call init_auth() first.")?;

        let url = self.base_url();
        debug!("Listing workflows: {}", url);

        let response = self.client
            .get(&url)
            .header("Authorization", Self::auth_header(&token))
            .query(&[("list_options", "{\"limit\": 50}")])
            .send()
            .await
            .context("Failed to list workflows")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("List workflows failed: {} - {}", status, body);
            anyhow::bail!("List workflows failed: {} - {}", status, body);
        }

        #[derive(Deserialize)]
        struct ListResponse {
            items: Vec<WorkflowListItem>,
        }

        #[derive(Deserialize)]
        struct WorkflowListItem {
            #[serde(rename = "metadata")]
            metadata: ListMetadata,
            #[serde(rename = "status")]
            status: ListStatus,
        }

        #[derive(Deserialize)]
        struct ListMetadata {
            name: String,
            namespace: String,
        }

        #[derive(Deserialize)]
        struct ListStatus {
            phase: String,
            started_at: Option<DateTime<Utc>>,
            finished_at: Option<DateTime<Utc>>,
        }

        let result: ListResponse = response.json().await
            .context("Failed to parse list response")?;

        let workflows: Vec<WorkflowSummary> = result.items
            .into_iter()
            .map(|item| WorkflowSummary {
                name: item.metadata.name,
                phase: parse_phase(&item.status.phase),
                namespace: item.metadata.namespace,
                started_at: item.status.started_at,
                finished_at: item.status.finished_at,
            })
            .collect();

        debug!("Listed {} workflows", workflows.len());

        Ok(workflows)
    }

    /// Delete a workflow by name.
    pub async fn delete_workflow(&self, name: &str) -> Result<()> {
        let token = self.get_token().await
            .context("No authentication token. Call init_auth() first.")?;

        let url = format!("{}/{}", self.base_url(), name);
        debug!("Deleting workflow: {}", url);

        let response = self.client
            .delete(&url)
            .header("Authorization", Self::auth_header(&token))
            .send()
            .await
            .context("Failed to delete workflow")?;

        // Argo returns 200 OK when successfully deleted
        if !response.status().is_success() && response.status().as_u16() != 404 {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("Delete workflow failed: {} - {}", status, body);
            anyhow::bail!("Delete workflow failed: {} - {}", status, body);
        }

        info!("Workflow deleted: {}", name);
        Ok(())
    }

    /// Watch a workflow for completion or termination.
    ///
    /// The callback is invoked each time the status changes.
    /// Returns when the workflow reaches a terminal state (Succeeded, Failed, Error).
    pub async fn watch_workflow<F>(&self, name: &str, mut callback: F) -> Result<WorkflowStatus>
    where
        F: FnMut(WorkflowStatus),
    {
        let mut poll_interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        let max_attempts = 360; // ~30 minutes max

        for attempt in 0..max_attempts {
            poll_interval.tick().await;

            match self.get_workflow_status(name).await {
                Ok(status) => {
                    debug!("Workflow {} status: {:?}", name, status.phase);
                    callback(status.clone());

                    // Check if workflow is in terminal state
                    match status.phase {
                        WorkflowPhase::Succeeded | WorkflowPhase::Failed | WorkflowPhase::Error => {
                            info!("Workflow {} reached terminal state: {:?}", name, status.phase);
                            return Ok(status);
                        }
                        _ => {}
                    }

                    // Handle archived/aborted workflows
                    if status.phase == WorkflowPhase::Archived {
                        info!("Workflow {} was archived", name);
                        return Ok(status);
                    }
                }
                Err(e) => {
                    error!("Error getting workflow status: {}", e);
                    // Continue polling on transient errors
                    if attempt >= max_attempts - 1 {
                        return Err(e);
                    }
                }
            }
        }

        anyhow::bail!("Workflow watch timeout after {} polls", max_attempts)
    }
}

/// Parse Argo workflow phase string to enum.
fn parse_phase(phase: &str) -> WorkflowPhase {
    match phase.to_lowercase().as_str() {
        "pending" => WorkflowPhase::Pending,
        "running" => WorkflowPhase::Running,
        "succeeded" => WorkflowPhase::Succeeded,
        "failed" => WorkflowPhase::Failed,
        "error" => WorkflowPhase::Error,
        "archived" => WorkflowPhase::Archived,
        _ => {
            warn!("Unknown workflow phase: {}", phase);
            WorkflowPhase::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_phase() {
        assert_eq!(parse_phase("Pending"), WorkflowPhase::Pending);
        assert_eq!(parse_phase("Running"), WorkflowPhase::Running);
        assert_eq!(parse_phase("Succeeded"), WorkflowPhase::Succeeded);
        assert_eq!(parse_phase("Failed"), WorkflowPhase::Failed);
        assert_eq!(parse_phase("error"), WorkflowPhase::Error);
        assert_eq!(parse_phase("unknown"), WorkflowPhase::Pending);
    }

    #[test]
    fn test_default_config() {
        let config = ArgoConfig::default();
        assert_eq!(config.namespace, "default");
        assert_eq!(config.service_account, "argo");
    }

    #[tokio::test]
    async fn test_workflow_without_auth() {
        let argo = ArgoWorkflow::default_client().unwrap();
        let result = argo.get_workflow_status("test").await;
        assert!(result.is_err()); // Should fail without auth
    }
}