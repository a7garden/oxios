import { test, expect } from '@playwright/test'

/**
 * E2E test for the Live Operations Dashboard (RFC-T1-C).
 *
 * Validates the basic structure: 5 KPI cards and Live Activity Feed.
 * Does not depend on any specific event being present in the SSE
 * stream — empty states are valid for a quiet system.
 */
test.describe('Live Operations Dashboard (RFC-T1-C)', () => {
  test('renders the five KPI stat cards', async ({ page }) => {
    await page.goto('/')
    await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible()

    // The 5 KPI labels are unique to the dashboard. They are rendered
    // inside the stat cards.
    await expect(page.getByText('Total Agents', { exact: true }).first()).toBeVisible()
    await expect(page.getByText('Running Agents', { exact: true }).first()).toBeVisible()
    await expect(page.getByText('Tokens/min', { exact: true }).first()).toBeVisible()
    await expect(page.getByText('CPU', { exact: true }).first()).toBeVisible()
    await expect(page.getByText('Pending Approvals', { exact: true }).first()).toBeVisible()
  })

  test('Live Activity Feed renders with connection state', async ({ page }) => {
    await page.goto('/')

    // The feed card heading is always shown.
    await expect(page.getByText(/Live Activity/i).first()).toBeVisible()

    // Filter dropdown + pause toggle are rendered in the card header.
    await expect(page.getByLabel('Filter events')).toBeVisible()
    await expect(page.getByLabel('Pause', { exact: false })).toBeVisible()
  })

})
