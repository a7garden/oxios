# Phase D: AgentBuilder 새 API 활용

> **위험**: 낮음 (additive only, 기존 코드 변경 없음)
> **예상 시간**: 3시간
> **선행**: Phase A

---

## 배경

oxi-sdk 0.26.0의 `AgentBuilder`에 새 메서드가 추가되었다:

```rust
// 신규 API
agent_builder.capabilities(caps)     // CapabilitySet 설정
agent_builder.authorizer(auth)       // Authorizer 부착
agent_builder.tracer(tracer)         // Tracer 부착
agent_builder.cost_tracker(ct)       // CostTracker 부착
agent_builder.audit_log(audit)       // AuditLog 부착
agent_builder.middleware(mw)         // 커스텀 미들웨어
agent_builder.kernel_tools(p, ctx)   // KernelToolProvider 플러그
```

현재 `agent_runtime.rs`의 `run_agent()` 함수는 `Agent::new_with_resolver()`를
직접 호출하고 middleware를 수동으로 연결한다. 새 API를 활용하면 이 과정이 간소화된다.

## 현재 코드 (간소화)

```rust
// agent_runtime.rs — run_agent() 내부
let agent = Arc::new(Agent::new_with_resolver(
    provider,
    agent_config,
    Arc::new(registry),
    resolver,
));

// middleware 수동 연결
if !pipeline.is_empty() {
    let terminate_flag = Arc::new(AtomicBool::new(false));
    let hooks = oxi_sdk::middleware::build_hooks(pipeline, agent_id, terminate_flag);
    agent.set_hooks(hooks);
}
```

## 변경 후

```rust
// engine.rs에 authorizer, tracer, cost_tracker 통합
pub struct OxiosEngine {
    oxi: Oxi,
    default_model_id: String,
    routing_control: Option<oxi_sdk::RoutingControl>,
    pools: ...,
    // ── 신규 ──
    authorizer: Option<Arc<oxi_sdk::Authorizer>>,
    tracer: Option<Arc<oxi_sdk::Tracer>>,
    cost_tracker: Option<Arc<oxi_sdk::CostTracker>>,
}

// agent_runtime.rs — AgentBuilder 사용
let mut builder = engine.oxi().agent(agent_config)
    .workspace(&workspace)
    .system_prompt(system_prompt);

// CSpace 기반 도구 등록 (기존 방식 유지)
register_tools_from_cspace_gated(&registry, ...);
// registry의 도구를 builder에 전달
for name in registry.names() {
    if let Some(tool) = registry.get(&name) {
        builder = builder.tool(/* ... */);
    }
}

// 0.26.0 새 API — 보안/관측 통합
if let Some(auth) = engine.authorizer() {
    builder = builder.authorizer(auth.clone());
}
if let Some(tracer) = engine.tracer() {
    builder = builder.tracer(tracer.clone());
}
if let Some(ct) = engine.cost_tracker() {
    builder = builder.cost_tracker(ct.clone());
}

// Middleware pipeline
if config.rate_limit_per_minute > 0 {
    builder = builder.with_rate_limit(config.rate_limit_per_minute);
}
if config.token_budget > 0 {
    builder = builder.with_token_budget(config.token_budget);
}
if config.audit_tool_calls {
    builder = builder.with_logging();
}

let agent = builder.build()?;
```

## 변경 파일

| 파일 | 변경 |
|------|------|
| `engine.rs` | `OxiosEngine`에 authorizer/tracer/cost_tracker 필드 추가 |
| `agent_runtime.rs` | `Agent::new_with_resolver` → `AgentBuilder` 사용 |

## 주의사항

1. **CSpace 기반 도구 등록은 그대로 유지**: `register_tools_from_cspace_gated()`는
   oxios 고유 로직이므로 AgentBuilder의 `.coding_tools()`로 대체하지 않는다.
2. **기존 middleware pipeline 호환성**: `build_hooks()` 직접 호출 대신
   AgentBuilder가 내부적으로 처리하도록 변경.
3. **Provider resolver**: AgentBuilder가 내부적으로 resolver를 생성하므로
   수동 `Arc<dyn ProviderResolver>` 생성 코드 제거 가능.

## 검증 기준

- [ ] `cargo test -p oxios-kernel` 통과
- [ ] AgentBuilder로 생성한 agent가 기존과 동일하게 동작
- [ ] tracer/cost_tracker에 데이터가 기록됨
