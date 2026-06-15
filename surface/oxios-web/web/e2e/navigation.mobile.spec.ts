import { expect, test } from '@playwright/test'
import { assertNoOverflow } from './helpers/overflow'

test.describe('Mobile Navigation', () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 })
    await page.goto('/')
    await page.waitForLoadState('networkidle')
  })

  test('sidebar drawer opens and closes on hamburger click', async ({ page }) => {
    // 햄버거 버튼은 모바일에서만 보임
    const hamburger = page.locator('button[aria-label="Open navigation menu"]')
    await expect(hamburger).toBeVisible()

    // 열기
    await hamburger.click()
    const drawer = page.locator('[role="dialog"]')
    await expect(drawer).toBeVisible()
    await expect(drawer).toContainText('Oxios')

    // 백드롭 클릭으로 닫기
    const backdrop = page.locator('[aria-hidden="false"]').first()
    if (await backdrop.isVisible()) {
      await backdrop.click({ force: true })
    }
    await expect(drawer).not.toBeVisible()
  })

  test('sidebar drawer closes on Escape', async ({ page }) => {
    const hamburger = page.locator('button[aria-label="Open navigation menu"]')
    await hamburger.click()
    const drawer = page.locator('[role="dialog"]')
    await expect(drawer).toBeVisible()

    await page.keyboard.press('Escape')
    await expect(drawer).not.toBeVisible()
  })

  test('navigates to agents page via drawer', async ({ page }) => {
    await page.locator('button[aria-label="Open navigation menu"]').click()
    await page.locator('[role="dialog"] a[href="/agents"]').click()
    await expect(page).toHaveURL('/agents')
  })

  test('no horizontal overflow on dashboard', async () => {
    // 이미 / 에 있음 (beforeEach)
    // overflow 검증은 별도 테스트에서 수행
  })
})

test.describe('Mobile Overflow', () => {
  test('no overflow on dashboard at 360px', async ({ page }) => {
    await page.goto('/')
    await assertNoOverflow(page, 360)
  })

  test('no overflow on dashboard at 768px', async ({ page }) => {
    await page.goto('/')
    await assertNoOverflow(page, 768)
  })

  test('no overflow on agents at 360px', async ({ page }) => {
    await page.goto('/agents')
    await assertNoOverflow(page, 360)
  })

  test('no overflow on knowledge at 360px', async ({ page }) => {
    await page.goto('/knowledge')
    await assertNoOverflow(page, 360)
  })

  test('no overflow on chat at 360px', async ({ page }) => {
    await page.goto('/chat')
    await assertNoOverflow(page, 360)
  })
})
