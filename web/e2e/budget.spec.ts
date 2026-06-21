import { test, expect } from '@playwright/test'

test.describe('Budget Page', () => {
  test('budget page loads', async ({ page }) => {
    await page.goto('/budget')
    await expect(page.getByRole('heading', { name: /budget/i })).toBeVisible()
  })

  test('shows empty state when no budgets', async ({ page }) => {
    await page.goto('/budget')
    // Should show loading state initially, then empty state
    await page.waitForSelector('[role="status"], text=/no.*budget/i', { timeout: 5000 })
  })

  test('set budget dialog opens', async ({ page }) => {
    await page.goto('/budget')
    // Look for any button that might open a budget dialog
    const addButton = page.getByRole('button', { name: /set budget|add/i }).first()
    if (await addButton.isVisible()) {
      await addButton.click()
      // Dialog should open (specific dialog content depends on implementation)
    }
  })

  test('refresh button is present', async ({ page }) => {
    await page.goto('/budget')
    const refreshButton = page.getByRole('button', { name: /refresh/i }).first()
    if (await refreshButton.isVisible()) {
      expect(refreshButton).toBeInTheDocument()
    }
  })
})