import { ArrowRight, ClipboardList } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";

/**
 * One structured question, as produced by the agent via the
 * `questionnaire` kernel tool. Mirrors the backend `Question` struct
 * from RFC-016.
 */
export interface QuestionnaireQuestion {
  id: string;
  prompt: string;
  kind: "single_choice" | "multi_choice" | "yes_no" | "free_text";
  options: { value: string; label: string; description?: string }[];
  allow_other: boolean;
}

interface QuestionnaireCardProps {
  /** Unique id of this questionnaire invocation. */
  questionnaireId: string;
  questions: QuestionnaireQuestion[];
  onSubmit: (result: {
    answers: { question_id: string; values: string[]; was_custom: boolean }[];
    cancelled: boolean;
  }) => void;
  disabled?: boolean;
}

/**
 * Claude-like interactive questionnaire card.
 *
 * Renders as a single self-contained card with:
 * - numbered questions, each with the appropriate widget
 * - a free-text "additional thoughts" area (always)
 * - Submit and Cancel buttons
 *
 * Behaves like pi-questionnaire: each question keeps local state;
 * Submit collects all answers into a single payload and calls onSubmit.
 */
export function QuestionnaireCard({
  questionnaireId: _questionnaireId,
  questions,
  onSubmit,
  disabled,
}: QuestionnaireCardProps) {
  const { t } = useTranslation();
  const [answers, setAnswers] = useState<Record<string, string[]>>({});
  const [freeText, setFreeText] = useState("");

  const setAnswer = (qid: string, values: string[]) => {
    setAnswers((prev) => ({ ...prev, [qid]: values }));
  };

  const toggleMulti = (qid: string, value: string) => {
    setAnswers((prev) => {
      const current = prev[qid] ?? [];
      const next = current.includes(value)
        ? current.filter((v) => v !== value)
        : [...current, value];
      return { ...prev, [qid]: next };
    });
  };

  const handleSubmit = () => {
    const payload = questions
      .map((q) => {
        const values = answers[q.id] ?? [];
        if (values.length === 0) return null;
        return { question_id: q.id, values, was_custom: false };
      })
      .filter((x): x is NonNullable<typeof x> => x !== null);
    onSubmit({ answers: payload, cancelled: false });
  };

  const handleCancel = () => {
    onSubmit({ answers: [], cancelled: true });
  };

  // All choice questions must have at least one selection.
  const allChoiceAnswered = questions
    .filter((q) => q.kind !== "free_text")
    .every((q) => (answers[q.id]?.length ?? 0) > 0);

  return (
    <div className="flex gap-3 my-1.5">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground">
        <ClipboardList className="h-4 w-4" />
      </div>
      <div className="max-w-[80%] flex-1">
        <div className="rounded-xl border bg-card shadow-sm">
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-3 border-b">
            <span className="text-sm font-medium">
              {t("questionnaire.title", "Questions")}
            </span>
            <span className="text-xs text-muted-foreground">
              {t("questionnaire.count", "{{count}} questions", {
                count: questions.length,
              })}
            </span>
          </div>

          {/* Questions */}
          <div className="p-4 space-y-5">
            {questions.map((q, i) => (
              <div key={q.id}>
                <p className="text-sm font-medium mb-2">
                  {i + 1}. {q.prompt}
                </p>
                <QuestionWidget
                  question={q}
                  value={answers[q.id]}
                  onChange={(vs) => setAnswer(q.id, vs)}
                  onToggle={(v) => toggleMulti(q.id, v)}
                  disabled={disabled}
                />
              </div>
            ))}

            {/* Free-text area for additional context (always available) */}
            <div>
              <p className="text-xs text-muted-foreground mb-1.5">
                {t(
                  "questionnaire.additionalThoughts",
                  "Additional thoughts (optional)",
                )}
              </p>
              <Textarea
                value={freeText}
                onChange={(e) => setFreeText(e.target.value)}
                placeholder={t(
                  "questionnaire.optionalPlaceholder",
                  "Anything else you want to add...",
                )}
                className="min-h-[60px] resize-none text-sm"
                disabled={disabled}
              />
            </div>
          </div>

          {/* Submit / Cancel */}
          <div className="flex justify-end gap-2 px-4 py-3 border-t">
            <Button
              onClick={handleCancel}
              variant="ghost"
              size="sm"
              disabled={disabled}
            >
              {t("questionnaire.cancel", "Cancel")}
            </Button>
            <Button
              onClick={handleSubmit}
              disabled={!allChoiceAnswered || disabled}
              size="icon"
              className="h-7 w-7"
              title={t('questionnaire.submit', 'Submit')}
            >
              <ArrowRight className="h-3.5 w-3.5" />
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}

function QuestionWidget({
  question,
  value,
  onChange,
  onToggle,
  disabled,
}: {
  question: QuestionnaireQuestion;
  value: string[] | undefined;
  onChange: (values: string[]) => void;
  onToggle: (value: string) => void;
  disabled?: boolean;
}) {
  const { t } = useTranslation();
  if (question.kind === "yes_no") {
    return (
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => onChange(["yes"])}
          disabled={disabled}
          className={cn(
            "px-4 py-1.5 rounded-lg text-sm border transition-colors",
            value?.includes("yes")
              ? "bg-success/15 text-success border-success/40"
              : "bg-card hover:bg-accent/50 border-border",
          )}
        >
          ✅ {t("chat.interview.yes")}
        </button>
        <button
          type="button"
          onClick={() => onChange(["no"])}
          disabled={disabled}
          className={cn(
            "px-4 py-1.5 rounded-lg text-sm border transition-colors",
            value?.includes("no")
              ? "bg-error/15 text-error border-error/40"
              : "bg-card hover:bg-accent/50 border-border",
          )}
        >
          ❌ {t("chat.interview.no")}
        </button>
      </div>
    );
  }

  if (question.kind === "multi_choice") {
    return (
      <div className="flex flex-wrap gap-2">
        {question.options.map((opt) => {
          const isActive = (value ?? []).includes(opt.value);
          return (
            <button
              key={opt.value}
              type="button"
              onClick={() => onToggle(opt.value)}
              disabled={disabled}
              className={cn(
                "px-3 py-1.5 rounded-full text-sm border transition-colors",
                isActive
                  ? "bg-primary text-primary-foreground border-primary"
                  : "bg-card hover:bg-accent/50 border-border",
              )}
            >
              {isActive ? "☑" : "☐"} {opt.label}
            </button>
          );
        })}
        {question.allow_other && (
          <input
            type="text"
            placeholder={t("questionnaire.typeSomething")}
            onChange={(e) => onChange(e.target.value ? [e.target.value] : [])}
            className="px-3 py-1.5 rounded-full text-sm border border-border bg-card min-w-[120px]"
            disabled={disabled}
          />
        )}
      </div>
    );
  }

  if (question.kind === "single_choice") {
    return (
      <div className="flex flex-wrap gap-2">
        {question.options.map((opt) => (
          <button
            key={opt.value}
            type="button"
            onClick={() => onChange([opt.value])}
            disabled={disabled}
            className={cn(
              "px-3 py-1.5 rounded-full text-sm border transition-colors",
              value?.[0] === opt.value
                ? "bg-primary text-primary-foreground border-primary"
                : "bg-card hover:bg-accent/50 border-border",
            )}
          >
            {opt.label}
          </button>
        ))}
        {question.allow_other && (
          <input
            type="text"
            placeholder={t("questionnaire.typeSomething")}
            onChange={(e) => onChange(e.target.value ? [e.target.value] : [])}
            className="px-3 py-1.5 rounded-full text-sm border border-border bg-card min-w-[120px]"
            disabled={disabled}
          />
        )}
      </div>
    );
  }

  // free_text
  return (
    <Textarea
      value={value?.[0] ?? ""}
      onChange={(e) => onChange(e.target.value ? [e.target.value] : [])}
      placeholder={t("questionnaire.typeAnswer")}
      className="min-h-[60px] resize-none text-sm"
      disabled={disabled}
    />
  );
}
