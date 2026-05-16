#!/usr/bin/env python3
"""
Oxios Agent Experience Benchmark — Runner

Usage:
    python run.py                    # 전체 실행
    python run.py --scenario s01     # 특정 시나리오만
    python run.py --judge-model anthropic/claude-sonnet-4   # Judge 모델 변경
    python run.py --dry-run          # 실행 없이 시나리오만 확인
    python run.py --verbose          # 상세 출력
"""

import argparse
import json
import os
import subprocess
import sys
import time
from dataclasses import dataclass, field, asdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

try:
    import tomllib
except ImportError:
    import tomli as tomllib

# ─── Paths ──────────────────────────────────────────────────────────────────
BENCH_DIR = Path(__file__).resolve().parent
SCENARIOS_DIR = BENCH_DIR / "scenarios"
REPORTS_DIR = BENCH_DIR / "reports"
JUDGE_PROMPT_TEMPLATE = BENCH_DIR / "templates" / "judge-prompt.md"
RUBRICS_FILE = BENCH_DIR / "judge" / "rubrics.toml"


# ─── Data Models ─────────────────────────────────────────────────────────────
@dataclass
class Scenario:
    """A single benchmark scenario loaded from TOML."""
    id: str
    name: str
    tier: int
    difficulty: str
    time_limit_secs: int
    goal: str  # What the user says to oxios
    min_expected: str = ""
    quality_expected: str = ""
    setup_files: list = field(default_factory=list)
    setup_commands: list = field(default_factory=list)
    follow_ups: list = field(default_factory=list)  # For multi-turn
    multi_step: bool = False  # If True, this is a multi-turn scenario


@dataclass
class StepResult:
    """One CLI invocation within a trajectory."""
    command: str
    exit_code: int
    duration_ms: int
    stdout: str
    stderr: str
    parsed: Optional[dict] = None


@dataclass
class Trajectory:
    """Full execution trace of a scenario."""
    scenario_id: str
    scenario_name: str
    goal: str
    started_at: str
    finished_at: str
    total_duration_ms: int
    steps: list = field(default_factory=list)  # List[StepResult]


@dataclass
class JudgeScore:
    """Score for one dimension."""
    score: int
    evidence: str


@dataclass
class JudgeResult:
    """Full judge evaluation for one scenario."""
    scenario_id: str
    scores: dict  # dimension -> JudgeScore
    weighted_total: float
    overall_assessment: str
    issues: list
    highlights: list


# ─── Scenario Loader ─────────────────────────────────────────────────────────
def load_scenario(path: Path) -> Scenario:
    with open(path, "rb") as f:
        data = tomllib.load(f)

    s = data["scenario"]
    e = data.get("evaluation", {})
    setup = data.get("setup", {})
    multi = data.get("multi_turn", {})

    return Scenario(
        id=s["id"],
        name=s["name"],
        tier=s["tier"],
        difficulty=s.get("difficulty", "medium"),
        time_limit_secs=s.get("time_limit_secs", 120),
        goal=s["goal"],
        min_expected=e.get("min_expected", ""),
        quality_expected=e.get("quality_expected", ""),
        setup_files=setup.get("files", []),
        setup_commands=setup.get("commands", []),
        follow_ups=multi.get("follow_ups", []),
        multi_step=multi.get("enabled", False),
    )


def load_all_scenarios(target_id: Optional[str] = None) -> list[Scenario]:
    scenarios = []
    for path in sorted(SCENARIOS_DIR.glob("*.toml")):
        s = load_scenario(path)
        if target_id and s.id != target_id:
            continue
        scenarios.append(s)
    return sorted(scenarios, key=lambda s: s.id)


# ─── Executor ────────────────────────────────────────────────────────────────
def run_setup(scenario: Scenario) -> None:
    """Run setup commands and create files before the scenario."""
    for file_def in scenario.setup_files:
        path = Path(os.path.expanduser(file_def["path"]))
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(file_def["content"])

    for cmd in scenario.setup_commands:
        subprocess.run(cmd, shell=True, capture_output=True, timeout=30)


def run_oxios(goal: str, session_id: Optional[str] = None,
              context_file: Optional[str] = None,
              timeout: int = 120) -> StepResult:
    """Execute a single oxios CLI call and record the result."""
    cmd_parts = ["oxios", "run", "--json"]

    if session_id:
        cmd_parts.extend(["--session", session_id])
    if context_file:
        cmd_parts.extend(["--context-file", context_file])

    cmd_parts.append(goal)
    cmd_str = " ".join(cmd_parts)

    start = time.monotonic()
    try:
        proc = subprocess.run(
            cmd_parts,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        exit_code = proc.returncode
        stdout = proc.stdout
        stderr = proc.stderr
    except subprocess.TimeoutExpired:
        exit_code = -1
        stdout = ""
        stderr = f"TIMEOUT after {timeout}s"
    except Exception as e:
        exit_code = -2
        stdout = ""
        stderr = str(e)

    elapsed_ms = int((time.monotonic() - start) * 1000)

    # Try to parse JSON
    parsed = None
    if stdout.strip():
        try:
            parsed = json.loads(stdout)
        except json.JSONDecodeError:
            pass

    return StepResult(
        command=cmd_str,
        exit_code=exit_code,
        duration_ms=elapsed_ms,
        stdout=stdout,
        stderr=stderr,
        parsed=parsed,
    )


def execute_scenario(scenario: Scenario) -> Trajectory:
    """Execute a full scenario (may be multi-turn) and record trajectory."""
    started = datetime.now(timezone.utc).isoformat()
    steps = []

    # Setup
    run_setup(scenario)

    # Find context_file if any setup files
    context_file = None
    if scenario.setup_files:
        context_file = os.path.expanduser(scenario.setup_files[0]["path"])

    # Turn 1
    step1 = run_oxios(
        scenario.goal,
        context_file=context_file,
        timeout=scenario.time_limit_secs,
    )
    steps.append(step1)

    # Multi-turn follow-ups
    if scenario.multi_step and step1.parsed:
        session_id = step1.parsed.get("session_id")
        for follow_up in scenario.follow_ups:
            step_n = run_oxios(
                follow_up,
                session_id=session_id,
                timeout=scenario.time_limit_secs,
            )
            steps.append(step_n)

    finished = datetime.now(timezone.utc).isoformat()
    total_ms = sum(s.duration_ms for s in steps)

    return Trajectory(
        scenario_id=scenario.id,
        scenario_name=scenario.name,
        goal=scenario.goal,
        started_at=started,
        finished_at=finished,
        total_duration_ms=total_ms,
        steps=steps,
    )


# ─── Judge ───────────────────────────────────────────────────────────────────
def build_judge_input(scenario: Scenario, trajectory: Trajectory) -> dict:
    """Build the structured input for the LLM judge."""
    steps_data = []
    for step in trajectory.steps:
        step_dict = {
            "command": step.command,
            "exit_code": step.exit_code,
            "duration_ms": step.duration_ms,
            "stderr": step.stderr[:500] if step.stderr else "",
        }
        if step.parsed:
            step_dict["response"] = step.parsed.get("response", "")[:2000]
            step_dict["evaluation_passed"] = step.parsed.get("evaluation_passed")
            step_dict["phase_reached"] = step.parsed.get("phase_reached")
            step_dict["session_id"] = step.parsed.get("session_id")
        else:
            step_dict["raw_stdout"] = step.stdout[:2000]
        steps_data.append(step_dict)

    return {
        "scenario": {
            "id": scenario.id,
            "name": scenario.name,
            "tier": scenario.tier,
            "goal": scenario.goal,
            "follow_ups": scenario.follow_ups,
            "min_expected": scenario.min_expected,
            "quality_expected": scenario.quality_expected,
        },
        "trajectory": {
            "total_duration_ms": trajectory.total_duration_ms,
            "num_steps": len(trajectory.steps),
            "steps": steps_data,
        },
    }


def call_judge(judge_input: dict, model: str) -> dict:
    """Call the LLM judge via oxios itself (meta!) or direct API."""
    prompt_template = JUDGE_PROMPT_TEMPLATE.read_text()
    rubrics = RUBRICS_FILE.read_text()

    judge_prompt = prompt_template.replace("{{RUBRICS}}", rubrics)
    judge_prompt += "\n\n## 평가 대상\n\n```json\n"
    judge_prompt += json.dumps(judge_input, ensure_ascii=False, indent=2)
    judge_prompt += "\n```\n\n위 데이터를 평가하여 JSON으로 응답하세요."

    # Use oxios to judge (meta-circular!)
    try:
        proc = subprocess.run(
            ["oxios", "run", "--json", judge_prompt],
            capture_output=True,
            text=True,
            timeout=120,
        )
        if proc.returncode == 0:
            result = json.loads(proc.stdout)
            response = result.get("response", "")
            # Extract JSON from response
            if "```json" in response:
                json_str = response.split("```json")[1].split("```")[0]
            elif "```" in response:
                json_str = response.split("```")[1].split("```")[0]
            else:
                json_str = response
            return json.loads(json_str.strip())
    except Exception as e:
        return {
            "error": str(e),
            "scores": {
                "completion": {"score": 0, "evidence": f"Judge failed: {e}"},
                "quality": {"score": 0, "evidence": "Judge failed"},
                "efficiency": {"score": 0, "evidence": "Judge failed"},
                "recovery": {"score": 0, "evidence": "Judge failed"},
            },
            "weighted_total": 0.0,
            "overall_assessment": f"Judge evaluation failed: {e}",
            "issues": [str(e)],
            "highlights": [],
        }


# ─── Reporter ────────────────────────────────────────────────────────────────
def compute_weighted_score(scores: dict) -> float:
    weights = {"completion": 0.40, "quality": 0.25, "efficiency": 0.15, "recovery": 0.20}
    total = 0.0
    for dim, weight in weights.items():
        s = scores.get(dim, {})
        score = s.get("score", 0) if isinstance(s, dict) else 0
        total += score * weight
    return round(total, 2)


def grade_from_score(score: float) -> str:
    if score >= 9.0: return "S"
    if score >= 8.0: return "A"
    if score >= 7.0: return "B"
    if score >= 6.0: return "C"
    return "D"


def generate_report(
    scenarios: list[Scenario],
    trajectories: list[Trajectory],
    judge_results: list[dict],
    report_dir: Path,
) -> Path:
    """Generate the final markdown report."""
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

    # Save raw data
    raw_dir = report_dir / "raw"
    raw_dir.mkdir(exist_ok=True)

    for traj in trajectories:
        traj_file = report_dir / f"trajectory-{traj.scenario_id}.json"
        traj_file.write_text(json.dumps(asdict(traj), ensure_ascii=False, indent=2))

    for jr in judge_results:
        if "scenario_id" in jr:
            jf = report_dir / f"judge-{jr['scenario_id']}.json"
        else:
            jf = report_dir / f"judge-error-{id(jr)}.json"
        jf.write_text(json.dumps(jr, ensure_ascii=False, indent=2))

    # Build summary
    total_weighted = 0.0
    count = 0
    tier_scores = {1: [], 2: [], 3: []}
    all_issues = []
    all_highlights = []

    rows = []
    for traj, jr in zip(trajectories, judge_results):
        scores = jr.get("scores", {})
        weighted = compute_weighted_score(scores)
        total_weighted += weighted
        count += 1

        # Find scenario tier
        scenario = next((s for s in scenarios if s.id == traj.scenario_id), None)
        tier = scenario.tier if scenario else 1
        tier_scores[tier].append(weighted)

        comp = scores.get("completion", {})
        qual = scores.get("quality", {})
        effi = scores.get("efficiency", {})
        recv = scores.get("recovery", {})

        rows.append({
            "id": traj.scenario_id,
            "name": traj.scenario_name,
            "completion": comp.get("score", "?") if isinstance(comp, dict) else "?",
            "quality": qual.get("score", "?") if isinstance(qual, dict) else "?",
            "efficiency": effi.get("score", "?") if isinstance(effi, dict) else "?",
            "recovery": recv.get("score", "?") if isinstance(recv, dict) else "?",
            "weighted": weighted,
            "duration_s": round(traj.total_duration_ms / 1000, 1),
        })

        all_issues.extend(jr.get("issues", []))
        all_highlights.extend(jr.get("highlights", []))

    avg_score = total_weighted / count if count else 0
    grade = grade_from_score(avg_score)

    tier_avgs = {}
    tier_names = {1: "일상 사용", 2: "oxios 기능", 3: "오류 복구"}
    for t, scores in tier_scores.items():
        tier_avgs[t] = round(sum(scores) / len(scores), 2) if scores else 0

    # Write report
    lines = [
        f"# Oxios Benchmark Report",
        f"",
        f"**일시:** {now}",
        f"**oxios 버전:** {_get_oxios_version()}",
        f"**Judge:** LLM-as-Judge (oxios run --json)",
        f"**시나리오 수:** {count}",
        f"",
        f"---",
        f"",
        f"## 종합 결과",
        f"",
        f"| 지표 | 값 |",
        f"|------|------|",
        f"| **총점** | **{avg_score:.2f}/10 ({grade})** |",
    ]

    for t in sorted(tier_avgs):
        g = grade_from_score(tier_avgs[t])
        lines.append(f"| Tier {t}: {tier_names[t]} | {tier_avgs[t]:.2f}/10 ({g}) |")

    total_duration = sum(t.total_duration_ms for t in trajectories)
    lines.extend([
        f"| 총 소요시간 | {total_duration / 1000:.1f}초 |",
        f"",
        f"---",
        f"",
        f"## 시나리오별 결과",
        f"",
        f"| ID | 이름 | Comp | Qual | Effi | Recv | 총점 | 시간 |",
        f"|----|------|------|------|------|------|------|------|",
    ])

    for r in rows:
        lines.append(
            f"| {r['id']} | {r['name']} | {r['completion']} | {r['quality']} "
            f"| {r['efficiency']} | {r['recovery']} | {r['weighted']:.1f} | {r['duration_s']}s |"
        )

    if all_issues:
        lines.extend([
            f"",
            f"---",
            f"",
            f"## 발견된 이슈 ({len(all_issues)})",
            f"",
        ])
        for issue in all_issues:
            lines.append(f"- {issue}")

    if all_highlights:
        lines.extend([
            f"",
            f"## 강점 ({len(all_highlights)})",
            f"",
        ])
        for h in all_highlights:
            lines.append(f"- {h}")

    lines.extend([
        "",
        "---",
        "",
        "## 상세 평가",
        "",
        "*각 시나리오의 Judge 판정은 `judge-sXX.json` 파일을 참조.*",
        "*실행 궤적은 `trajectory-sXX.json` 파일을 참조.*",
    ])

    report_path = report_dir / "summary.md"
    report_path.write_text("\n".join(lines))
    return report_path


def _get_oxios_version() -> str:
    try:
        proc = subprocess.run(["oxios", "--version"], capture_output=True, text=True, timeout=5)
        return proc.stdout.strip() or "unknown"
    except Exception:
        return "unknown"


# ─── Main ────────────────────────────────────────────────────────────────────
def main():
    parser = argparse.ArgumentParser(description="Oxios Agent Experience Benchmark")
    parser.add_argument("--scenario", help="Run only this scenario (e.g., s01)")
    parser.add_argument("--judge-model", default="anthropic/claude-sonnet-4", help="Judge model")
    parser.add_argument("--dry-run", action="store_true", help="Show scenarios without running")
    parser.add_argument("--verbose", action="store_true", help="Verbose output")
    parser.add_argument("--no-judge", action="store_true", help="Skip judge evaluation")
    args = parser.parse_args()

    # Load scenarios
    scenarios = load_all_scenarios(args.scenario)
    if not scenarios:
        print("❌ No scenarios found.")
        sys.exit(1)

    print(f"📋 Loaded {len(scenarios)} scenarios")

    if args.dry_run:
        for s in scenarios:
            tier_name = {1: "기본", 2: "기능", 3: "오류"}[s.tier]
            multi = " (multi-turn)" if s.multi_step else ""
            print(f"  {s.id}: [{tier_name}] {s.name}{multi}")
            print(f"      Goal: {s.goal[:80]}...")
        return

    # Create report directory
    timestamp = datetime.now().strftime("%Y-%m-%d-%H%M%S")
    report_dir = REPORTS_DIR / f"benchmark-{timestamp}"
    report_dir.mkdir(parents=True)

    # Execute
    trajectories = []
    judge_results = []

    for i, scenario in enumerate(scenarios):
        print(f"\n{'='*60}")
        print(f"  [{i+1}/{len(scenarios)}] {scenario.id}: {scenario.name}")
        print(f"  Goal: {scenario.goal}")
        print(f"{'='*60}")

        # Execute
        traj = execute_scenario(scenario)
        trajectories.append(traj)

        # Print results
        for j, step in enumerate(traj.steps):
            turn = f" (turn {j+1})" if len(traj.steps) > 1 else ""
            status = "✅" if step.exit_code == 0 else "❌"
            print(f"  {status} {step.command[:60]}... ({step.duration_ms}ms){turn}")
            if args.verbose and step.parsed:
                resp = step.parsed.get("response", "")
                print(f"     Response: {resp[:150]}...")
            if step.stderr:
                print(f"     Stderr: {step.stderr[:100]}")

        # Judge
        if not args.no_judge:
            judge_input = build_judge_input(scenario, traj)
            jr = call_judge(judge_input, args.judge_model)
            jr["scenario_id"] = scenario.id
            judge_results.append(jr)

            wt = compute_weighted_score(jr.get("scores", {}))
            print(f"  📊 Score: {wt:.1f}/10")
        else:
            judge_results.append({
                "scenario_id": scenario.id,
                "scores": {},
                "weighted_total": 0,
                "overall_assessment": "Skipped (--no-judge)",
                "issues": [],
                "highlights": [],
            })

    # Report
    print(f"\n{'='*60}")
    print("📊 Generating report...")
    report_path = generate_report(scenarios, trajectories, judge_results, report_dir)
    print(f"✅ Report: {report_path}")

    # Summary
    if judge_results and not args.no_judge:
        scores = [compute_weighted_score(jr.get("scores", {})) for jr in judge_results]
        avg = sum(scores) / len(scores)
        grade = grade_from_score(avg)
        print(f"\n{'='*60}")
        print(f"  Total: {avg:.2f}/10 ({grade})")
        print(f"  Scenarios: {len(scenarios)}")
        print(f"{'='*60}")


if __name__ == "__main__":
    main()
