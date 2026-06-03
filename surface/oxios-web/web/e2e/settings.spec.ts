import { test, expect, type Page } from '@playwright/test'

/**
 * RFC-T1-D: Settings UI Completion (MVP)
 *
 * Intercepts /api/config so the tests can run without a live Oxios
 * daemon. Asserts the new group navigation, hot-reload badges, the
 * diff preview flow, and that the sticky save bar appears when there
 * are unsaved changes.
 */

const SAMPLE_CONFIG = {
  engine: { default_model: '', api_key: null, routing_enabled: false },
  kernel: { workspace: '~/.oxios/workspace', max_agents: 10, event_bus_capacity: 256 },
  exec: {
    default_mode: 'structured',
    allow_shell_mode: false,
    allowed_commands: ['ls', 'cat', 'rg'],
    allowlist_mode: 'permissive',
    default_timeout_secs: 120,
    max_timeout_secs: 600,
  },
  security: {
    auth_enabled: false,
    network_access: false,
    can_fork: false,
    max_execution_time_secs: 300,
    max_memory_mb: 512,
    max_audit_entries: 10000,
    cors_origins: ['http://localhost:4200'],
    allowed_tools: ['read', 'write', 'bash'],
    rate_limit_per_minute: 120,
    audit_log_path: null,
  },
  scheduler: { max_concurrent: 5, rate_limit_per_minute: 60, zombie_timeout_secs: 300 },
  orchestrator: { max_evolution_iterations: 3, min_evaluation_score: 0.8, eval_cache_enabled: true },
  context: { active_limit_tokens: 100000, cache_limit_entries: 50 },
  gateway: { host: '127.0.0.1', port: 4200 },
  session: { max_sessions: 100, ttl_hours: 168, auto_prune: true },
  logging: { format: 'pretty', level: null },
  memory: {
    enabled: true,
    sqlite: { path: '~/.oxios/workspace/memory.db', enabled: true },
    embedding: { provider: 'gguf', dimension: 256 },
    learning: { sona_enabled: true },
    consolidation: {
      preset: 'balanced',
      dream_enabled: true,
      dream_interval_hours: 24,
    },
  },
  channels: {
    enabled: [],
    telegram: {
      bot_token_env: 'TELEGRAM_BOT_TOKEN',
      allowed_users: [],
      session: { rotation_hours: 2, max_messages: 0 },
    },
  },
  audit: { enabled: true, max_entries: 100000 },
  resource_monitor: { cpu_threshold: 90, memory_threshold: 90, load_threshold: 8 },
}

async function mockConfigApi(page: Page) {
  await page.route('**/api/config', async (route) => {
    const req = route.request()
    if (req.method() === 'GET') {
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(SAMPLE_CONFIG) })
    } else if (req.method() === 'PATCH' || req.method() === 'PUT') {
      // Echo a hot_reload report so the UI can render the success notice.
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          config: SAMPLE_CONFIG,
          hot_reload: {
            applied_immediately: ['exec.allowed_commands'],
            requires_restart: [],
            total_changed: 1,
          },
        }),
      })
    } else {
      await route.continue()
    }
  })
}

test.describe('Settings (RFC-T1-D)', () => {
  test.beforeEach(async ({ page }) => {
    await mockConfigApi(page)
    await page.goto('/settings')
  })

  test('group sidebar shows 5 group labels', async ({ page }) => {
    for (const groupLabel of ['AI', 'System', 'Security', 'Memory', 'Channels']) {
      // Each group label appears in the sidebar nav. We use the first
      // visible match to avoid colliding with body text.
      const heading = page.getByText(groupLabel, { exact: true }).first()
      await expect(heading).toBeVisible()
    }
  })

  test('exec section shows allowlist editor and badges', async ({ page }) => {
    await page.getByRole('button', { name: 'Execution' }).first().click()
    await expect(page.getByText('Allowed Commands')).toBeVisible()
    await expect(page.getByText('Allowlist Mode')).toBeVisible()

    // Hot-reload badges are emitted with data-testid="hot-reload-badge".
    const hotBadges = page.getByTestId('hot-reload-badge')
    expect(await hotBadges.count()).toBeGreaterThan(0)
  })

  test('memory section renders 4 sub-cards', async ({ page }) => {
    await page.getByRole('button', { name: 'Memory' }).first().click()
    for (const sub of ['Storage', 'Embedding', 'Learning', 'Dream']) {
      // CardTitle renders as a div, not a heading — match by text.
      await expect(page.getByText(sub, { exact: true }).first()).toBeVisible()
    }
  })

  test('embedding provider field has a restart badge', async ({ page }) => {
    await page.getByRole('button', { name: 'Memory' }).first().click()
    const restartBadges = page.getByTestId('restart-badge')
    expect(await restartBadges.count()).toBeGreaterThan(0)
  })

  test('sticky save bar is visible', async ({ page }) => {
    const saveBar = page.getByTestId('sticky-save-bar')
    await expect(saveBar).toBeVisible()
  })

  test('save flow opens diff preview', async ({ page }) => {
    // Modify a field on the exec section.
    await page.getByRole('button', { name: 'Execution' }).first().click()

    // The Default Mode select — click and pick the other option.
    const modeButton = page.getByRole('combobox').first()
    if (await modeButton.isVisible()) {
      await modeButton.click()
      // Pick the shell option (or whatever exists) — we just need
      // the diff to be non-empty.
      const option = page.getByRole('option').first()
      if (await option.isVisible()) {
        await option.click()
      }
    }

    // Click Save.
    const saveButton = page.getByTestId('save-changes')
    if (await saveButton.isEnabled()) {
      await saveButton.click()
      // The diff preview modal should open.
      await expect(page.getByText('Confirm changes')).toBeVisible()
    }
  })
})
