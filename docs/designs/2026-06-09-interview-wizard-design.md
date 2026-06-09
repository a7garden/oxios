# Interview Wizard — Interactive Questionnaire UI

> **Date:** 2026-06-09
> **Status:** Approved
> **Scope:** Replace `InterviewResponse` with a Claude-like step-by-step wizard
> **Affects:** `surface/oxios-web/web/src/components/chat/`

---

## 1. Motivation

The Ouroboros protocol's interview phase asks clarifying questions before seed generation.
The current `InterviewResponse` component renders all questions in a single scrollable card,
which creates answer fatigue and doesn't match the interactive UX users expect from Claude's
web interface. This design replaces it with a step-by-step wizard that shows one question at
a time with always-visible free-text input.

## 2. UX Model

**Claude-like Wizard** — one question per step, with navigation, clarity gauge, and always-visible free-text.

### 2.1 Visual Layout

```
┌─────────────────────────────────────────────────────┐
│  🔍 인터뷰 라운드 1        Clarity ██████░░ 62%     │
│  ───────────────────────────────────────────────────│
│                                                     │
│  1 of 3                                             │
│                                                     │
│  "최적화 목표가 무엇인가요?"                          │
│                                                     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐            │
│  │ 🚀 속도  │ │ 💰 비용  │ │ ⚖️ 균형  │            │
│  └──────────┘ └──────────┘ └──────────┘            │
│                                                     │
│  ┌──────────────────────────────────────────┐      │
│  │  또는 직접 입력...                        │      │
│  └──────────────────────────────────────────┘      │
│                                                     │
│  ───────────────────────────────────────────────────│
│           [ 건너뛰기 ]         [ 다음 → ]           │
└─────────────────────────────────────────────────────┘
```

### 2.2 Mobile Layout

- Option buttons stack vertically
- Free-text input sticks to bottom
- Large touch-friendly navigation buttons
- Clarity gauge collapses to a thin bar

## 3. Component Architecture

```
InterviewWizard (replaces InterviewResponse)
├── WizardHeader        — round indicator + clarity gauge + progress
├── QuestionStep        — current question widget
│   ├── SingleChoice    — pill/chip button grid
│   ├── MultiChoice     — toggleable pill buttons
│   ├── YesNo           — large Yes/No card buttons
│   └── FreeText        — textarea (when kind=free_text)
├── FreeTextInput       — always-visible direct input area
└── WizardFooter        — prev/next/skip/submit buttons
```

### 3.1 InterviewWizard

**Props:**
```ts
interface InterviewWizardProps {
  questions: InterviewQuestion[]
  round: number
  ambiguity: number
  onSubmit: (answers: InterviewAnswer[]) => void
  disabled?: boolean
}
```

**Local state:**
- `currentStep: number` (0-indexed)
- `answers: Record<string, string | string[]>` (question id → value)
- `freeText: string` (per-step free-text)
- `globalFreeText: string` (overall additional thoughts, submitted with last step)

**Behavior:**
- On mount, shows question at `currentStep = 0`
- Advances on "Next", goes back on "Prev"
- On last question's "Next" (shows as "제출"), collects all answers and calls `onSubmit`
- Free-text input is per-step; if typed, it overrides/supplements the structured selection
- Submit formats answers as `InterviewAnswer[]` and passes to parent

### 3.2 WizardHeader

**Renders:**
- Round label: "인터뷰 라운드 {round}"
- Clarity gauge: `clarity = 1 - ambiguity`, colored bar (red → yellow → green)
- Progress dots: ● ● ○ (filled = answered, empty = unanswered)
- Question counter: "{current + 1} of {total}"

### 3.3 QuestionStep Widgets

#### SingleChoice
- Pill/chip buttons in a flex-wrap grid
- Selected = `bg-primary text-primary-foreground` + checkmark icon
- Option descriptions shown below selected option (fade-in)
- Keyboard: `1-9` for quick selection

#### MultiChoice
- Toggleable pill buttons
- Selected count badge: "2 선택됨"
- `☑` / `☐` prefix per option

#### YesNo
- Two large card-style buttons
- Yes = green-tinted (`bg-success/15`), No = red-tinted (`bg-error/15`)
- ✅ / ❌ emoji prefix

#### FreeText (kind=free_text)
- Textarea with `min-h: 80px`
- No structured options

### 3.4 FreeTextInput

- Always visible below the structured widget
- Placeholder: "또는 직접 입력..."
- Label: subtle, "또는 직접 답변을 입력하세요"
- When user types here, structured selection is preserved (merged on submit)
- Uses the project's `Textarea` UI component

### 3.5 WizardFooter

**Buttons:**
- **이전 (←)**: shown when `currentStep > 0`, icon: ArrowLeft
- **건너뛰기**: always available (allows skipping a question)
- **다음 →** / **제출**: advances or submits on last step, icon: ArrowRight

**Keyboard shortcuts:**
- `Enter` = Next/Submit (when not in textarea focus)
- `Backspace` = Prev (when not in textarea focus)
- `1-9` = Select option by number (SingleChoice/MultiChoice only)
- `Escape` = Skip current question

## 4. Multi-Round Continuous Cycle

When the user submits answers and the backend decides more clarification is needed:

1. Submit → `submitInterviewResponse()` in chat store
2. Store sends WS `interview_response` message
3. Backend runs another interview pass
4. New `interview` chunk arrives with `round: 2`, updated `ambiguity`
5. Chat store updates `activeInterview`, `interviewRound`, `interviewAmbiguity`
6. `InterviewWizard` re-mounts with new questions + new round
7. Between rounds: brief "Clarity가 {X}%로 향상되었습니다" feedback message

When `ambiguity ≤ 0.2`:
- Backend proceeds to seed generation
- No more interview chunks
- Chat store clears `activeInterview`
- Chat input reappears, agent begins processing

## 5. Graceful Degradation

When `structured_questions` is `null` or empty (LLM didn't produce structured output):

1. Fall back to rendering the plain `response` markdown text
2. Show a single free-text input below
3. User types their answer naturally
4. No wizard navigation (single step)

This preserves the existing Orchestrator behavior where structured output is best-effort.

## 6. Chat Store Changes

**Minimal changes** — reuse existing state:

| Field | Current | Change |
|-------|---------|--------|
| `activeInterview` | `InterviewQuestion[] \| null` | No change |
| `interviewRound` | `number` | No change |
| `interviewAmbiguity` | `number` | No change |
| `submitInterviewResponse()` | Builds answer text, sends WS | No change |
| New: `interviewHistory` | — | `Array<{round, questions, answers, ambiguity}>` for round-to-round feedback |

## 7. Files Changed

### New files
- `components/chat/interview-wizard.tsx` — main wizard component
- `components/chat/wizard-step.tsx` — step wrapper with animation

### Modified files
- `routes/chat.tsx` — replace `InterviewResponse` with `InterviewWizard`
- `stores/chat.ts` — add `interviewHistory` for round tracking

### Removed files
- `components/chat/interview-response.tsx` — replaced by `interview-wizard.tsx`
- `components/chat/interview-question-card.tsx` — inlined into wizard step widgets

### Unchanged
- `components/chat/questionnaire-card.tsx` — separate RFC-016 tool, not affected
- Backend (Rust) — no changes needed
- Types — `InterviewQuestion`, `InterviewAnswer` reused as-is

## 8. Testing

- Unit tests: `InterviewWizard` rendering with each question kind
- Integration: multi-round interview cycle via mock WS chunks
- Accessibility: keyboard navigation, screen reader labels
- Mobile: responsive breakpoint testing (640px, 768px, 1024px)
