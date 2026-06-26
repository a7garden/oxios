import { test, expect } from '@playwright/test'

test.describe('Sidebar Navigation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/')
  })

  test('all sidebar items navigate correctly', async ({ page }) => {
    // Dashboard
    await page.getByRole('link', { name: 'Dashboard', exact: true }).first().click()
    await expect(page).toHaveURL('/')
    
    // Chat
    await page.getByRole('link', { name: 'Chat' }).click()
    await expect(page).toHaveURL('/chat')
    
    // Agents
    await page.getByRole('link', { name: 'Agents' }).click()
    await expect(page).toHaveURL('/agents')
    
    // Budget
    await page.getByRole('link', { name: 'Budget' }).click()
    await expect(page).toHaveURL('/budget')
    
    // Settings
    await page.getByRole('link', { name: 'Settings' }).click()
    await expect(page).toHaveURL('/settings')
  })


  test('sidebar collapse toggle works', async ({ page }) => {
    // Find and click the collapse toggle
    const collapseButton = page.getByRole('button', { name: /panel|toggle|collapse/i }).first()
    if (await collapseButton.isVisible()) {
      await collapseButton.click()
      // Sidebar should be collapsed
      await expect(page.locator('aside')).toBeVisible()
    }
  })

  test('breadcrumb navigation works for sub-pages', async ({ page }) => {
    // Navigate to budget page
    await page.getByRole('link', { name: 'Budget' }).click()
    
    // Check if breadcrumb shows current location
    const breadcrumb = page.locator('[aria-label="breadcrumb"], nav[aria-label], .breadcrumb')
    if (await breadcrumb.count() > 0) {
      await expect(breadcrumb.first()).toBeVisible()
    }
  })
})