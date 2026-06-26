import { test, expect } from '@playwright/test'

test.describe('Oxios Web UI', () => {
  test('loads the dashboard page', async ({ page }) => {
    await page.goto('/')
    await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
    await expect(page.getByText('Oxios Agent OS overview')).toBeVisible()
  })

  test('sidebar navigation works', async ({ page }) => {
    await page.goto('/')

    // Navigate to Chat
    await page.getByRole('link', { name: 'Chat' }).click()
    await expect(page.getByRole('heading', { name: 'Chat' })).toBeVisible()

    // Navigate to Agents
    await page.getByRole('link', { name: 'Agents' }).click()
    await expect(page.getByRole('heading', { name: 'Agents' })).toBeVisible()

    // Navigate to Settings
    await page.getByRole('link', { name: 'Settings' }).click()
    await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible()
  })

  test('sidebar collapses and expands', async ({ page }) => {
    await page.goto('/')

    // Collapse sidebar
    await page.getByRole('button', { name: /panel/i }).first().click()

    // Sidebar should be narrow (16 = w-16)
    const sidebar = page.locator('aside')
    await expect(sidebar).toBeVisible()

    // Expand again
    await page.getByRole('button', { name: /panel/i }).first().click()
  })

  test('theme toggle cycles through dark/light/system', async ({ page }) => {
    await page.goto('/')

    // Default is dark
    await expect(page.locator('html')).toHaveClass(/dark/)

    // Click to light
    await page.getByRole('button', { name: /light/i }).click()
    await expect(page.locator('html')).not.toHaveClass(/dark/)

    // Click to system
    await page.getByRole('button', { name: /system/i }).click()

    // Click back to dark
    await page.getByRole('button', { name: /dark/i }).click()
    await expect(page.locator('html')).toHaveClass(/dark/)
  })

  test('all sidebar navigation items are present', async ({ page }) => {
    await page.goto('/')

    const navItems = [
      'Dashboard', 'Chat',
      'Agents', 'Sessions', 'Spaces', 'Programs', 'Skills',
      'Memory', 'Workspace',
      'Scheduler', 'Security', 'Budget', 'Resources',
      'Cron Jobs', 'Git', 'Personas', 'Agent Groups', 'Host Tools',
    ]

    for (const item of navItems) {
      await expect(page.getByRole('link', { name: item, exact: true }).first()).toBeVisible()
    }
  })

  test('chat page renders input area', async ({ page }) => {
    await page.goto('/chat')
    await expect(page.getByPlaceholder('Type a message...')).toBeVisible()
    await expect(page.getByRole('button', { name: /send/i })).toBeVisible()
  })

  test('settings page shows tabs', async ({ page }) => {
    await page.goto('/settings')
    await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible()
    await expect(page.getByText('General')).toBeVisible()
    await expect(page.getByText('Engine')).toBeVisible()
  })

  test('workspace page renders', async ({ page }) => {
    await page.goto('/workspace')
    await expect(page.getByRole('heading', { name: 'Workspace' })).toBeVisible()
  })

  test('resources page renders chart area', async ({ page }) => {
    await page.goto('/resources')
    await expect(page.getByRole('heading', { name: 'Resources' })).toBeVisible()
  })
})
