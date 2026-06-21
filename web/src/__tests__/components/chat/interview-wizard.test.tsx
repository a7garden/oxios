import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { InterviewWizard } from '@/components/chat/interview-wizard'
import type { InterviewQuestion } from '@/types'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

const singleChoice = (id: string, text: string): InterviewQuestion => ({
  id,
  text,
  kind: 'single_choice',
  options: [
    { value: 'a', label: 'Option A' },
    { value: 'b', label: 'Option B' },
  ],
})

const multiChoice = (id: string, text: string): InterviewQuestion => ({
  id,
  text,
  kind: 'multi_choice',
  options: [
    { value: 'x', label: 'X' },
    { value: 'y', label: 'Y' },
  ],
})

// Helper: dispatch a key on document.body so the wizard's window-level
// listener receives it via bubbling.
const press = (key: string, shiftKey = false) => fireEvent.keyDown(document.body, { key, shiftKey })

describe('InterviewWizard keyboard operability', () => {
  it('Enter advances to the next step', () => {
    render(
      <InterviewWizard
        questions={[singleChoice('q1', 'First question?'), singleChoice('q2', 'Second question?')]}
        round={1}
        ambiguity={0.5}
        onSubmit={vi.fn()}
      />,
    )

    expect(screen.getByText('First question?')).toBeInTheDocument()
    expect(screen.queryByText('Second question?')).not.toBeInTheDocument()

    // Enter alone advances — no click, no focus juggling required.
    press('Enter')

    expect(screen.queryByText('First question?')).not.toBeInTheDocument()
    expect(screen.getByText('Second question?')).toBeInTheDocument()
  })

  it('Enter on the last step submits the collected answers', () => {
    const onSubmit = vi.fn()
    render(
      <InterviewWizard
        questions={[singleChoice('q1', 'Only question?')]}
        round={1}
        ambiguity={0.5}
        onSubmit={onSubmit}
      />,
    )

    // Pick option 1 via number key, then Enter to submit.
    press('1')
    press('Enter')

    expect(onSubmit).toHaveBeenCalledTimes(1)
    const submitted = onSubmit.mock.calls[0]![0] as Array<{ question_id: string; value: string }>
    expect(submitted).toHaveLength(1)
    expect(submitted[0]!.question_id).toBe('q1')
    expect(submitted[0]!.value).toBe('a')
  })

  it('number keys select single-choice options without a mouse', () => {
    const onSubmit = vi.fn()
    render(
      <InterviewWizard
        questions={[singleChoice('q1', 'Pick one?')]}
        round={1}
        ambiguity={0.5}
        onSubmit={onSubmit}
      />,
    )

    press('2') // selects Option B (value 'b')
    press('Enter')

    const submitted = onSubmit.mock.calls[0]![0] as Array<{ value: string }>
    expect(submitted[0]!.value).toBe('b')
  })

  it('number keys toggle multi-choice options and submit all selected', () => {
    const onSubmit = vi.fn()
    render(
      <InterviewWizard
        questions={[multiChoice('q1', 'Pick many?')]}
        round={1}
        ambiguity={0.5}
        onSubmit={onSubmit}
      />,
    )

    press('1') // toggle X
    press('2') // toggle Y
    press('Enter')

    const submitted = onSubmit.mock.calls[0]![0] as Array<{ value: string }>
    // Both selections survive into the submitted value (joined with '; ').
    expect(submitted[0]!.value).toContain('x')
    expect(submitted[0]!.value).toContain('y')
  })

  it('Escape skips the current step (advances without recording an answer)', () => {
    const onSubmit = vi.fn()
    render(
      <InterviewWizard
        questions={[singleChoice('q1', 'Skip me?')]}
        round={1}
        ambiguity={0.5}
        onSubmit={onSubmit}
      />,
    )

    press('Escape')

    // On the last step, skip submits whatever is collected — here nothing,
    // so onSubmit is called with an empty list.
    expect(onSubmit).toHaveBeenCalledTimes(1)
    expect(onSubmit.mock.calls[0]![0]).toHaveLength(0)
  })
  it('Enter inside the free-text textarea advances to the next step', () => {
    render(
      <InterviewWizard
        questions={[singleChoice('q1', 'First question?'), singleChoice('q2', 'Second question?')]}
        round={1}
        ambiguity={0.5}
        onSubmit={vi.fn()}
      />,
    )

    // The always-visible free-text textarea is the realistic failure point:
    // previously Enter inserted a newline and did NOT advance. Now it advances.
    const textarea = screen.getByPlaceholderText('chat.interview.orType')
    textarea.focus()
    fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: false })

    expect(screen.queryByText('First question?')).not.toBeInTheDocument()
    expect(screen.getByText('Second question?')).toBeInTheDocument()
  })

  it('Shift+Enter inside the textarea does NOT advance (inserts a newline)', () => {
    render(
      <InterviewWizard
        questions={[singleChoice('q1', 'First question?'), singleChoice('q2', 'Second question?')]}
        round={1}
        ambiguity={0.5}
        onSubmit={vi.fn()}
      />,
    )

    const textarea = screen.getByPlaceholderText('chat.interview.orType')
    textarea.focus()
    fireEvent.keyDown(textarea, { key: 'Enter', shiftKey: true })

    // Still on the first step — Shift+Enter is reserved for a newline.
    expect(screen.getByText('First question?')).toBeInTheDocument()
    expect(screen.queryByText('Second question?')).not.toBeInTheDocument()
  })
  it('does not advance while disabled (mid-stream)', () => {
    render(
      <InterviewWizard
        questions={[singleChoice('q1', 'First question?'), singleChoice('q2', 'Second question?')]}
        round={1}
        ambiguity={0.5}
        onSubmit={vi.fn()}
        disabled
      />,
    )

    press('Enter')

    // Still on the first step — keyboard must respect the disabled lock.
    expect(screen.getByText('First question?')).toBeInTheDocument()
    expect(screen.queryByText('Second question?')).not.toBeInTheDocument()
  })
  it('arrow keys move and select for single-choice (radiogroup behavior)', () => {
    const onSubmit = vi.fn()
    render(
      <InterviewWizard
        questions={[singleChoice('q1', 'Pick one?')]}
        round={1}
        ambiguity={0.5}
        onSubmit={onSubmit}
      />,
    )

    const radios = screen.getAllByRole('radio')
    expect(radios).toHaveLength(2)
    radios[0]!.focus()
    // Arrow both moves focus AND selects — native radiogroup semantics.
    press('ArrowRight')
    press('Enter')

    const submitted = onSubmit.mock.calls[0]![0] as Array<{ value: string }>
    expect(submitted[0]!.value).toBe('b')
  })

  it('single-choice options expose radiogroup + radio semantics', () => {
    render(
      <InterviewWizard
        questions={[singleChoice('q1', 'Pick one?')]}
        round={1}
        ambiguity={0.5}
        onSubmit={vi.fn()}
      />,
    )

    expect(screen.getByRole('radiogroup')).toBeInTheDocument()
    const radios = screen.getAllByRole('radio')
    // Nothing selected yet → both unchecked.
    expect(radios[0]).toHaveAttribute('aria-checked', 'false')
    expect(radios[1]).toHaveAttribute('aria-checked', 'false')

    // Selecting the second flips its aria-checked (re-query: React re-renders).
    fireEvent.click(radios[1]!)
    const updated = screen.getAllByRole('radio')
    expect(updated[1]).toHaveAttribute('aria-checked', 'true')
    expect(updated[0]).toHaveAttribute('aria-checked', 'false')
  })

  it('multi-choice options expose toggle (aria-pressed) semantics', () => {
    render(
      <InterviewWizard
        questions={[multiChoice('q1', 'Pick many?')]}
        round={1}
        ambiguity={0.5}
        onSubmit={vi.fn()}
      />,
    )

    expect(screen.getByRole('group')).toBeInTheDocument()
    const toggles = screen.getAllByRole('button').filter((b) => b.hasAttribute('aria-pressed'))
    expect(toggles).toHaveLength(2)
    expect(toggles[0]).toHaveAttribute('aria-pressed', 'false')
    fireEvent.click(toggles[0]!)
    expect(
      screen.getAllByRole('button').filter((b) => b.hasAttribute('aria-pressed'))[0],
    ).toHaveAttribute('aria-pressed', 'true')
  })
})
