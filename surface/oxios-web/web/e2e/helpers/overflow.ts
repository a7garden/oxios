import { expect, type Page } from '@playwright/test'

/**
 * 주어진 폭에서 가로 오버플로우가 없는지 검증.
 * 1. documentElement scrollWidth 비교
 * 2. overflow-x: hidden으로 위장된 잘린 콘텐츠 감지
 */
export async function assertNoOverflow(page: Page, width = 360) {
  await page.setViewportSize({ width, height: 640 })
  await page.waitForLoadState('networkidle')

  const result = await page.evaluate(() => {
    const doc = document.documentElement
    const hasHorizontalScroll = doc.scrollWidth > doc.clientWidth

    const clipped = Array.from(document.body.querySelectorAll<HTMLElement>('*')).filter(el => {
      const style = getComputedStyle(el)
      const hidesOverflow = style.overflowX === 'hidden' || style.overflowX === 'clip'
      if (!hidesOverflow) return false
      return el.scrollWidth > el.clientWidth + 1
    })

    return {
      hasHorizontalScroll,
      clippedCount: clipped.length,
      clippedSamples: clipped.slice(0, 3).map(e => e.className || e.tagName),
    }
  })

  expect(result.hasHorizontalScroll, `${width}px에서 가로 스크롤 발생`).toBe(false)
  expect(result.clippedCount, `${width}px에서 ${result.clippedCount}개 요소 오버플로우 숨김 (${result.clippedSamples.join(', ')})`).toBe(0)
}

/** 핵심 브레이크포인트 전체 검증 */
export async function assertNoOverflowAllBreakpoints(page: Page) {
  for (const w of [360, 390, 768, 1024]) {
    await assertNoOverflow(page, w)
  }
}
