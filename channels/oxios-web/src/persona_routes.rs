//! Persona API routes: CRUD and active persona management.
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::server::AppState;

// ---------------------------------------------------------------------------
// Personas
// ---------------------------------------------------------------------------

/// Persona summary for listing.
#[derive(Debug, Serialize)]
pub struct PersonaSummary {
    id: String,
    name: String,
    role: String,
    description: String,
    enabled: bool,
    personality_traits: Vec<String>,
}

/// GET /api/personas — List all personas.
pub async fn handle_personas_list(
    state: State<Arc<AppState>>,
) -> Json<Vec<PersonaSummary>> {
    let personas = state.kernel.persona.list();
    Json(personas
        .into_iter()
        .map(|p| PersonaSummary {
            id: p.id,
            name: p.name,
            role: p.role,
            description: p.description,
            enabled: p.enabled,
            personality_traits: p.personality_traits,
        })
        .collect())
}

/// GET /api/personas/:id — Get a specific persona.
pub async fn handle_persona_get(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match state.kernel.persona.get(&id) {
        Some(p) => Ok(Json(serde_json::json!({
            "id": p.id,
            "name": p.name,
            "role": p.role,
            "description": p.description,
            "system_prompt": p.system_prompt,
            "enabled": p.enabled,
            "model": p.model,
            "personality_traits": p.personality_traits,
        }))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// Request body for creating a persona.
#[derive(Debug, Deserialize)]
pub struct PersonaCreateRequest {
    name: String,
    role: String,
    description: String,
    #[serde(default)]
    system_prompt: String,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    personality_traits: Vec<String>,
}

fn default_true() -> bool {
    true
}

/// POST /api/personas — Create a new persona.
pub async fn handle_persona_create(
    state: State<Arc<AppState>>,
    Json(body): Json<PersonaCreateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use oxios_kernel::Persona;
    let persona = Persona {
        id: uuid::Uuid::new_v4().to_string(),
        name: body.name,
        role: body.role,
        description: body.description,
        system_prompt: body.system_prompt,
        enabled: body.enabled,
        model: body.model,
        personality_traits: body.personality_traits,
    };
    state.kernel.persona.create(persona.clone());
    tracing::info!(persona = %persona.name, "Persona created via API");
    Ok(Json(serde_json::json!({
        "status": "created",
        "id": persona.id,
        "name": persona.name,
    })))
}

/// Request body for updating a persona.
#[derive(Debug, Deserialize)]
pub struct PersonaUpdateRequest {
    name: Option<String>,
    role: Option<String>,
    description: Option<String>,
    system_prompt: Option<String>,
    enabled: Option<bool>,
    model: Option<String>,
    personality_traits: Option<Vec<String>>,
}

/// PUT /api/personas/:id — Update a persona.
pub async fn handle_persona_update(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<PersonaUpdateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use oxios_kernel::Persona;
    let existing = state.kernel.persona.get(&id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Persona '{}' not found", id)))?;

    let updated = Persona {
        id: existing.id,
        name: body.name.unwrap_or(existing.name),
        role: body.role.unwrap_or(existing.role),
        description: body.description.unwrap_or(existing.description),
        system_prompt: body.system_prompt.unwrap_or(existing.system_prompt),
        enabled: body.enabled.unwrap_or(existing.enabled),
        model: body.model.or(existing.model),
        personality_traits: body.personality_traits.unwrap_or(existing.personality_traits),
    };

    state.kernel.persona.update(&id, updated.clone())
        .map_err(|e: anyhow::Error| (StatusCode::BAD_REQUEST, e.to_string()))?;
    tracing::info!(persona_id = %id, "Persona updated via API");
    Ok(Json(serde_json::json!({
        "status": "updated",
        "id": id,
    })))
}

/// DELETE /api/personas/:id — Delete a persona.
pub async fn handle_persona_delete(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Prevent deleting the last persona.
    if state.kernel.persona.count() <= 1 {
        return Err((StatusCode::BAD_REQUEST, "Cannot delete the last persona".to_string()));
    }

    state.kernel.persona.delete(&id)
        .map_err(|e: anyhow::Error| (StatusCode::NOT_FOUND, e.to_string()))?;

    // If deleted persona was active, clear the active reference.
    if let Some(active) = state.kernel.persona.active() {
        if active.id == id {
            // Try to set another persona as active.
            if let Some(next) = state.kernel.persona.list_enabled().into_iter().next() {
                let _ = state.kernel.persona.set_active(&next.id);
            }
        }
    }

    tracing::info!(persona_id = %id, "Persona deleted via API");
    Ok(Json(serde_json::json!({
        "status": "deleted",
        "id": id,
    })))
}

/// GET /api/personas/active — Get the currently active persona.
pub async fn handle_persona_active_get(
    state: State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    match state.kernel.persona.active() {
        Some(p) => Json(serde_json::json!({
            "id": p.id,
            "name": p.name,
            "role": p.role,
            "description": p.description,
            "system_prompt": p.system_prompt,
            "enabled": p.enabled,
        })),
        None => Json(serde_json::json!({
            "active": false,
            "message": "No active persona set"
        })),
    }
}

/// Request body for setting active persona.
#[derive(Debug, Deserialize)]
pub struct PersonaActiveRequest {
    id: String,
}

/// PUT /api/personas/active — Set the active persona.
pub async fn handle_persona_active_set(
    state: State<Arc<AppState>>,
    Json(body): Json<PersonaActiveRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    state.kernel.persona.set_active(&body.id)
        .map_err(|e: anyhow::Error| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let persona = state.kernel.persona.active();
    Ok(Json(serde_json::json!({
        "status": "active",
        "id": body.id,
        "name": persona.map(|p| p.name).unwrap_or_default(),
    })))
}