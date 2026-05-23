//! 실제 LLM을 사용한 Ouroboros Interview 시나리오 테스트.
//!
//! 실행: cargo test -p oxios-ouroboros --test scenario_test -- --nocapture --test-threads=1
//!
//! 각 시나리오에 대해 LLM이 어떻게 분류하는지 (is_task, ambiguity, questions) 확인.
//! 모호한 요청에 대해 interview가 제대로 질문을 던지는지 검증.

use std::sync::Arc;

use oxi_sdk::{OxiBuilder, Provider};
use oxios_ouroboros::{OuroborosEngine, OuroborosProtocol};

/// auth.json에서 zai API 키 읽기.
fn load_zai_key() -> Option<String> {
    let path = std::env::var("OXI_AUTH_PATH")
        .unwrap_or_else(|_| format!("{}/.oxi/auth.json", std::env::var("HOME").unwrap_or_default()));
    let content = std::fs::read_to_string(&path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("zai")?
        .get("access_token")?
        .as_str()
        .map(|s| s.to_string())
}

/// 테스트용 OuroborosEngine 생성.
async fn make_engine() -> Arc<dyn OuroborosProtocol> {
    let api_key = load_zai_key().expect("zai API key not found in ~/.oxi/auth.json");
    let base_url = std::env::var("ZAI_BASE_URL")
        .unwrap_or_else(|_| "https://api.z.ai/api/coding/paas/v4".to_string());

    let key_for_closure = api_key.clone();
    let builder = OxiBuilder::new().with_builtins();
    let builder = builder.provider_factory("zai", move || {
        let provider =
            oxi_ai::OpenAiProvider::with_base_url_and_key(&base_url, Some(key_for_closure.clone()));
        Ok(Arc::new(provider) as Arc<dyn Provider>)
    });

    let oxi = builder.build();
    let model = oxi.resolve_model("zai/glm-5-turbo").expect("model not found");
    let provider = oxi.create_provider("zai").expect("provider creation failed");
    Arc::new(OuroborosEngine::new(provider, model))
}

/// 시나리오 정의.
struct Scenario {
    name: &'static str,
    message: &'static str,
    /// 예상 결과 (None = 검증 스킵)
    expected_is_task: Option<bool>,
    /// None = 스킵, Some(true) = ambiguity ≤ 0.2, Some(false) = ambiguity > 0.2
    expected_ready: Option<bool>,
    /// None = 스킵, Some(true) = 질문 있음, Some(false) = 질문 없음
    expected_has_questions: Option<bool>,
}

fn scenarios() -> Vec<Scenario> {
    vec![
        // ── Category 1: 명확한 작업 요청 ──
        Scenario {
            name: "명확한 파일 수정",
            message: "src/main.rs에서 greet 함수를 hello로 이름 변경해줘",
            expected_is_task: Some(true),
            expected_ready: Some(true),
            expected_has_questions: Some(false),
        },
        Scenario {
            name: "명확한 CLI 실행",
            message: "cargo test 실행해줘",
            expected_is_task: Some(true),
            expected_ready: Some(true),
            expected_has_questions: Some(false),
        },

        // ── Category 2: 모호한 작업 요청 ──
        Scenario {
            name: "모호한 수정",
            message: "이거 좀 고쳐줘",
            expected_is_task: Some(true),
            expected_ready: Some(false),
            expected_has_questions: Some(true),
        },
        Scenario {
            name: "모호한 버그 리포트",
            message: "앱이 느려",
            expected_is_task: Some(true),
            expected_ready: Some(false),
            expected_has_questions: Some(true),
        },
        Scenario {
            name: "모호한 개선 요청",
            message: "여기 좀 예쁘게 해봐",
            expected_is_task: Some(true),
            expected_ready: Some(false),
            expected_has_questions: Some(true),
        },
        Scenario {
            name: "모호한 배포",
            message: "배포해줘",
            expected_is_task: Some(true),
            expected_ready: Some(false),
            expected_has_questions: Some(true),
        },

        // ── Category 3: 대화/비작업 ──
        Scenario {
            name: "인사",
            message: "안녕",
            expected_is_task: Some(false),
            expected_ready: None,
            expected_has_questions: None,
        },
        Scenario {
            name: "감사",
            message: "고마워",
            expected_is_task: Some(false),
            expected_ready: None,
            expected_has_questions: None,
        },
        Scenario {
            name: "일반 질문",
            message: "Rust에서 소유권이 뭐야?",
            expected_is_task: Some(false),
            expected_ready: None,
            expected_has_questions: None,
        },

        // ── Category 4: 경계 케이스 ──
        Scenario {
            name: "부분 구체성 (파일+모호한 수정)",
            message: "로그인 페이지 수정해줘",
            expected_is_task: Some(true),
            expected_ready: None,  // 애매함 — 어떻게 나올지 보기
            expected_has_questions: None,
        },
        Scenario {
            name: "구체적 설명+모호한 요청",
            message: "dashboard.tsx에 Chart 컴포넌트가 데이터를 못 불러오는 것 같아. 확인해줘",
            expected_is_task: Some(true),
            expected_ready: None,  // 파일은 있는데 "확인"이 모호
            expected_has_questions: None,
        },
    ]
}

/// 결과 예쁘게 출력 + 검증
fn print_and_verify(name: &str, s: &Scenario, result: &oxios_ouroboros::InterviewResult) -> (usize, usize) {
    let mut pass = 0;
    let mut fail = 0;

    let is_task_str = if result.is_task { "TASK" } else { "CHAT" };
    let ambiguity = result.ambiguity.ambiguity();
    let ready = result.ready_for_seed;
    let n_questions = result.questions.iter().filter(|q| !q.is_empty()).count();

    println!("─────────────────────────────────────────");
    println!("📌 {}", name);
    println!("   입력: \"{}\"", s.message);

    // is_task 검증
    print!("   분류: {}", is_task_str);
    if let Some(exp) = s.expected_is_task {
        let ok = result.is_task == exp;
        print!(" {} (expected: {})", if ok { "✅" } else { "❌" }, if exp { "TASK" } else { "CHAT" });
        if ok { pass += 1; } else { fail += 1; }
    }
    println!();

    // Ambiguity
    println!("   Ambiguity: {:.3}  ready={}  (goal={:.2} constraint={:.2} criteria={:.2})",
        ambiguity, ready,
        result.ambiguity.goal_clarity,
        result.ambiguity.constraint_clarity,
        result.ambiguity.success_criteria);

    if let Some(exp_ready) = s.expected_ready {
        let ok = ready == exp_ready;
        println!("   {} ready: actual={}, expected={}",
            if ok { "✅" } else { "❌" }, ready, exp_ready);
        if ok { pass += 1; } else { fail += 1; }
    }

    // Questions
    if let Some(exp_q) = s.expected_has_questions {
        let has_q = n_questions > 0;
        let ok = has_q == exp_q;
        println!("   {} has_questions: actual={}, expected={}",
            if ok { "✅" } else { "❌" }, has_q, exp_q);
        if ok { pass += 1; } else { fail += 1; }
    }

    if n_questions > 0 {
        println!("   질문:");
        for (i, q) in result.questions.iter().enumerate() {
            if !q.is_empty() {
                println!("     {}. {}", i + 1, q);
            }
        }
    }

    if !result.is_task && !result.chat_response.is_empty() {
        let truncated: String = result.chat_response.chars().take(120).collect();
        println!("   챗 응답: \"{}\"", truncated);
    }

    println!();
    (pass, fail)
}

#[tokio::test]
async fn test_interview_scenarios() {
    let engine = make_engine().await;
    let scenarios = scenarios();
    let mut total_pass = 0;
    let mut total_fail = 0;

    println!("\n══════════════════════════════════════════════════════════════");
    println!("  Ouroboros Interview 시나리오 테스트 (실제 LLM)");
    println!("══════════════════════════════════════════════════════════════\n");

    for s in &scenarios {
        match engine.interview(s.message).await {
            Ok(result) => {
                let (p, f) = print_and_verify(s.name, s, &result);
                total_pass += p;
                total_fail += f;
            }
            Err(e) => {
                println!("─────────────────────────────────────────");
                println!("📌 {} — ❌ LLM 오류: {}", s.name, e);
                println!();
                total_fail += 1;
            }
        }
    }

    println!("══════════════════════════════════════════════════════════════");
    println!("  총 검증: ✅ {} 통과  ❌ {} 실패", total_pass, total_fail);
    println!("══════════════════════════════════════════════════════════════\n");

    if total_fail > 0 {
        // Print summary of failures for quick scanning
        println!("⚠️  실패가 있습니다. 위 출력에서 ❌를 확인하세요.");
    }
}
