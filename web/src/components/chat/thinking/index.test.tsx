import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { Thinking } from './index'

describe('Thinking shiny title', () => {
  it('applies the shiny animation class to the title while thinking', () => {
    render(<Thinking content="some reasoning" thinking />)
    // i18n is stubbed to return keys in tests, so match by the shiny class on
    // the title span rather than its (localized) text.
    expect(document.querySelector('.thinking-shiny')).not.toBeNull()
  })

  it('does not apply the shiny class once thinking is done', () => {
    render(<Thinking content="some reasoning" thinking={false} duration={1200} />)
    expect(document.querySelector('.thinking-shiny')).toBeNull()
  })
})
