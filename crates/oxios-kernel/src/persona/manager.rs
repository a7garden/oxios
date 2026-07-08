//! Persona manager: coordinates persona-aware execution.
//!
//! The PersonaManager manages persona lifecycle and provides
//! the active persona for orchestrator and agent runtime.

use anyhow::Result;
use parking_lot::RwLock;

use super::store::PersonaStore;
use super::{Persona, default_personas};

/// Manages persona lifecycle and coordinates persona-aware execution.
#[derive(Debug)]
pub struct PersonaManager {
    store: PersonaStore,
    active_persona_id: RwLock<Option<String>>,
}

impl PersonaManager {
    /// Creates a new persona manager with default personas.
    pub fn new() -> Self {
        let store = PersonaStore::new();
        let manager = Self {
            store,
            active_persona_id: RwLock::new(None),
        };
        manager.create_default_personas();
        manager
    }

    /// Creates a new persona manager, optionally loading from existing data.
    pub fn with_defaults(personas: Vec<Persona>) -> Self {
        let store = PersonaStore::new();
        store.load_from_slice(&personas);
        let this = Self {
            store,
            active_persona_id: RwLock::new(None),
        };
        // Set the first enabled persona as active by default.
        if let Some(first) = this.store.list_enabled().into_iter().next() {
            *this.active_persona_id.write() = Some(first.id);
        }
        this
    }

    /// Returns the current active persona, if any.
    pub fn get_active_persona(&self) -> Option<Persona> {
        let active_id = self.active_persona_id.read().clone();
        active_id.and_then(|id| self.store.get(&id))
    }

    /// Sets the active persona by ID.
    pub fn set_active_persona(&self, id: &str) -> Result<()> {
        // Verify the persona exists and is enabled.
        let persona = self
            .store
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Persona '{id}' not found"))?;
        if !persona.enabled {
            anyhow::bail!("Persona '{id}' is disabled");
        }
        *self.active_persona_id.write() = Some(id.to_string());
        tracing::info!(persona_id = %id, name = %persona.name, "Active persona set");
        Ok(())
    }

    /// Returns the system prompt for the active persona.
    /// Falls back to a default prompt if no active persona.
    pub fn active_system_prompt(&self) -> String {
        self.get_active_persona()
            .map(|p| p.system_prompt.clone())
            .unwrap_or_else(|| {
                "You are a helpful AI assistant that follows the Ouroboros methodology: \
                 specify before you build, evaluate before you ship."
                    .to_string()
            })
    }

    /// Creates the three default personas (Dev, Review, Research).
    pub fn create_default_personas(&self) {
        let defaults = default_personas();
        for persona in defaults {
            // Only register if not already present.
            if self.store.get(&persona.id).is_none() {
                self.store.register(persona);
            }
        }
        // Set first persona as active if none is set.
        {
            let mut active = self.active_persona_id.write();
            if active.is_none() {
                *active = Some("dev".to_string());
            }
        }
        tracing::info!("Default personas initialized");
    }

    /// Returns the first enabled persona, for wiring into OuroborosEngine.
    pub fn first_enabled(&self) -> Option<Persona> {
        self.store.list_enabled().into_iter().next()
    }

    /// Returns the persona store for direct access.
    pub fn store(&self) -> &PersonaStore {
        &self.store
    }

    /// Returns the ID of the active persona.
    pub fn active_persona_id(&self) -> Option<String> {
        self.active_persona_id.read().clone()
    }

    // ── RFC-039 ────────────────────────────────────────────────────────────

    /// Active persona 의 우선순위 결정:
    ///   1. StateStore `index.json` 의 `active_persona_id` (enabled 면 적용)
    ///   2. `PersonaConfig.default_persona_id` (enabled 면 적용)
    ///   3. store 의 첫 번째 enabled
    ///   4. None
    ///
    /// `&self` — `Arc<PersonaManager>` 뒤에서 호출됨.
    pub fn apply_config(&self, cfg: &crate::config::PersonaConfig) {
        // 우선순위 1: 기존 active_persona_id 가 enabled 면 유지
        // (load_from_state_store 가 이미 StateStore 의 active_persona_id 를 박았음).
        if let Some(id) = self.active_persona_id() {
            if self
                .store
                .get(&id)
                .map(|p| p.enabled)
                .unwrap_or(false)
            {
                return;
            }
        }
        // 우선순위 2: config.default_persona_id
        if let Some(id) = cfg.default_persona_id.as_ref() {
            if self
                .store
                .get(id)
                .map(|p| p.enabled)
                .unwrap_or(false)
            {
                *self.active_persona_id.write() = Some(id.clone());
                return;
            }
        }
        // 우선순위 3: 첫 번째 enabled
        if let Some(p) = self.store.list_enabled().into_iter().next() {
            *self.active_persona_id.write() = Some(p.id);
        }
    }

    /// StateStore 에서 페르소나 + active_persona_id 를 로드.
    /// 손상·부재 시 silent fallback 하지 않고 `Result::Err` 로 전파.
    /// 호출자는 defaults 가 이미 new() 로 박혀 있음을 알고 있어야 함.
    pub async fn load_from_state_store(
        &self,
        store: &crate::state_store::StateStore,
    ) -> Result<()> {
        let snap = crate::persona::persistence::load_from_state_store(store)
            .await?
            .ok_or_else(|| anyhow::anyhow!("persona: no snapshot present"))?;
        // store 가 이미 기본 페르소나를 들고 있어도 디스크가 우선.
        for p in &snap.personas {
            self.store.register(p.clone());
        }
        if let Some(active) = snap.active_persona_id {
            if snap.personas.iter().any(|p| p.id == active && p.enabled) {
                *self.active_persona_id.write() = Some(active);
            }
        }
        Ok(())
    }

    /// StateStore 에 페르소나 + active_persona_id 를 저장.
    /// 메모리 상태는 유지, IO 실패는 Result 로 전파.
    pub async fn persist(
        &self,
        store: &crate::state_store::StateStore,
    ) -> Result<()> {
        let snapshot = crate::persona::persistence::PersonaSnapshot {
            schema_version: 1,
            active_persona_id: self.active_persona_id(),
            personas: self.store.list_all(),
        };
        crate::persona::persistence::save_to_state_store(store, &snapshot).await
    }

    /// 글로벌 활성 페르소나 변경. 성공 시 슬롯 변경 + StateStore flush.
    /// 새 system_prompt 를 `Ok(Some(prompt))` 로 반환 — 호출자가
    /// `IntentEngine::set_persona_prompt` 로 직접 재시드 (kernel ↔ ouroboros
    /// 의존성 방향 회피).
    ///
    /// `&self` — interior mutability (Arc 뒤 호출 가능).
    pub async fn set_active(
        &self,
        id: &str,
        store: Option<&crate::state_store::StateStore>,
    ) -> Result<Option<String>> {
        let persona = self
            .store
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Persona '{id}' not found"))?;
        if !persona.enabled {
            anyhow::bail!("Persona '{id}' is disabled");
        }
        *self.active_persona_id.write() = Some(id.to_string());
        tracing::info!(persona_id = %id, name = %persona.name, "Active persona set");
        if let Some(s) = store {
            self.persist(s).await?;
        }
        Ok(Some(persona.system_prompt))
    }
}

// IntentReseed 트레이트는 의도적으로 두지 않는다 — 위 set_active docstring 참조.
// (oxios-ouroboros 는 oxios-kernel 에 의존하지 않으므로 트레이트를 kernel 안에
//  둘 수 없음. 대신 set_active 가 새 system_prompt 를 Ok(Some(prompt)) 로
//  반환하고, 호출자가 IntentEngine::set_persona_prompt 를 직접 호출.)

impl Default for PersonaManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for PersonaManager {
    fn clone(&self) -> Self {
        let personas: Vec<Persona> = self.store.list_all();
        let store = PersonaStore::new();
        store.load_from_slice(&personas);
        Self {
            store,
            active_persona_id: RwLock::new(self.active_persona_id.read().clone()),
        }
    }
}
