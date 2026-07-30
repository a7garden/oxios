import {
  autocompletion,
  type CompletionContext,
  type CompletionResult,
} from '@codemirror/autocomplete'
import { EditorState } from '@codemirror/state'
import { describe, expect, it } from 'vitest'

const noopSource = (_ctx: CompletionContext): CompletionResult | null => null

/**
 * Regression guard for the knowledge-base editor crash
 * "Config merge conflict for field override".
 *
 * `@atomic-editor/editor`'s `wikiLinks()` bundles its own `autocompletion({ override })`
 * when its `suggest` option is set, and `@codemirror/autocomplete`'s completion
 * facet has no per-field merge combiner for `override`
 * (see `combineConfig` → throws "Config merge conflict for field " + key). So at
 * most ONE `autocompletion({ override })` may appear in a single editor config;
 * that single extension's `override` array must carry every completion source.
 *
 * `wikiLinks()` is a third-party function (the package's exports map blocks a
 * deep import in the test runner), so these tests pin the CodeMirror constraint
 * directly — it is the exact mechanism `wikiLinks()` triggers internally
 * (dist/wiki-links.js: `autocompletion({ override: [source] })`).
 */
describe('markdown-editor autocompletion assembly', () => {
  it('throws when two autocompletion({ override }) share one config', () => {
    expect(() =>
      EditorState.create({
        extensions: [
          autocompletion({ override: [noopSource] }),
          autocompletion({ override: [noopSource] }),
        ],
      }),
    ).toThrow(/Config merge conflict for field override/)
  })

  it('does not throw when a single autocompletion carries all override sources', () => {
    expect(() =>
      EditorState.create({
        extensions: [autocompletion({ override: [noopSource, noopSource] })],
      }),
    ).not.toThrow()
  })
})
