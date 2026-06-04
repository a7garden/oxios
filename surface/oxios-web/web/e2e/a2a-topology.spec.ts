import { test, expect, type Page } from '@playwright/test'

/**
 * RFC-T1-A: A2A Interactive Topology (MVP e2e).
 *
 * Mocks the /api/a2a/* endpoints so the page can be exercised
 * without a live Oxios daemon. The mock includes two agents and
 * one edge so the topology canvas renders something interactive.
 */

const AGENT_A = {
  agent_id: '00000000-0000-0000-0000-00000000000a',
  name: 'agent-alpha',
  description: 'Primary worker',
  capabilities: ['code-review'],
  skills: ['rust'],
  status: 'running',
  endpoint: 'local',
}

const AGENT_B = {
  agent_id: '00000000-0000-0000-0000-00000000000b',
  name: 'agent-beta',
  description: 'Reviewer',
  capabilities: ['lint', 'format'],
  skills: ['python'],
  status: 'idle',
  endpoint: 'local',
}

const TOPOLOGY = {
  nodes: [
    {
      id: 'agent-alpha',
      label: 'agent-alpha',
      status: 'running',
      capabilities: ['code-review'],
      skills: ['rust'],
      last_seen: new Date().toISOString(),
    },
    {
      id: 'agent-beta',
      label: 'agent-beta',
      status: 'idle',
      capabilities: ['lint', 'format'],
      skills: ['python'],
      last_seen: new Date().toISOString(),
    },
  ],
  edges: [
    {
      from: 'agent-alpha',
      to: 'agent-beta',
      message_count_5m: 3,
      last_kind: 'task_delegation',
    },
  ],
}

// Mirror the kernel's `A2AMessageLogEntry.from/to` storage: the log
// records AgentId (UUID), not agent name. The backend's
// `/api/a2a/messages` route transforms UUIDs to names via its
// `name_map` before responding. The mock must do the same
// transformation so the response shape mirrors the real wire format.
const NAME_MAP: Record<string, string> = {
  [AGENT_A.agent_id]: AGENT_A.name,
  [AGENT_B.agent_id]: AGENT_B.name,
}

function nameOf(uuid: string): string {
  return NAME_MAP[uuid] ?? uuid
}

const RAW_LOG_ENTRIES: Array<{
  from: string
  to: string
  message_type: string
  content: string
}> = [
  {
    from: AGENT_A.agent_id,
    to: AGENT_B.agent_id,
    message_type: 'task_delegation',
    content: 'Review the PR',
  },
  {
    from: AGENT_B.agent_id,
    to: AGENT_A.agent_id,
    message_type: 'status_update',
    content: '50% complete',
  },
]

const MESSAGES = {
  messages: RAW_LOG_ENTRIES.map((entry, i) => ({
    request_id: `req-${i + 1}`,
    from_agent: nameOf(entry.from),
    to_agent: nameOf(entry.to),
    message_type: entry.message_type,
    payload_summary: entry.content,
    accepted: true,
    timestamp: new Date().toISOString(),
  })),
}

async function mockA2aApi(page: Page) {
  await page.route('**/api/a2a/agents', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ agents: [AGENT_A, AGENT_B] }),
    })
  })
  await page.route('**/api/a2a/agents/*', async (route) => {
    const url = route.request().url()
    const id = url.split('/').pop() ?? ''
    const card = [AGENT_A, AGENT_B].find((a) => a.agent_id === id) ?? AGENT_A
    await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(card) })
  })
  await page.route('**/api/a2a/messages', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(MESSAGES),
    })
  })
  await page.route('**/api/a2a/topology', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(TOPOLOGY),
    })
  })
}

test.describe('A2A Interactive Topology (RFC-T1-A)', () => {
  test.beforeEach(async ({ page }) => {
    await mockA2aApi(page)
    await page.goto('/a2a')
  })

  test('renders the ReactFlow topology canvas with agent nodes', async ({ page }) => {
    // Page heading
    await expect(page.getByRole('heading', { name: 'A2A Protocol Monitor' })).toBeVisible()

    // The Topology tab is the default. Wait for the canvas.
    const canvas = page.getByTestId('a2a-topology-canvas')
    await expect(canvas).toBeVisible()

    // The custom AgentNode renders an aria-label like
    // "Agent agent-alpha, status running". Verify both nodes.
    const alpha = page.getByLabel('Agent agent-alpha, status running')
    const beta = page.getByLabel('Agent agent-beta, status idle')
    await expect(alpha).toBeVisible()
    await expect(beta).toBeVisible()
  })

  test('opens the inspector when an agent node is clicked', async ({ page }) => {
    const alpha = page.getByLabel('Agent agent-alpha, status running')
    await alpha.click()

    // The inspector is a dialog with the agent name as title.
    const inspector = page.getByTestId('a2a-agent-inspector')
    await expect(inspector).toBeVisible()

    // Inspector shows the agent name and a capability.
    await expect(inspector.getByText('agent-alpha').first()).toBeVisible()
    await expect(inspector.getByText('code-review').first()).toBeVisible()

    // Last messages list is rendered (the mock has 2 messages).
    const messages = inspector.getByTestId('a2a-inspector-messages')
    await expect(messages).toBeVisible()
    const items = messages.locator('li')
    expect(await items.count()).toBeGreaterThan(0)

    // Press Escape to close.
    await page.keyboard.press('Escape')
    // The inspector animates out; wait for the data-testid to disappear.
    await expect(inspector).not.toBeVisible()
  })

  test('switches to Messages tab and shows message log', async ({ page }) => {
    await page.getByRole('button', { name: 'Messages' }).first().click()
    // The MessageLog renders a table with task_delegation row.
    await expect(page.getByText('task_delegation').first()).toBeVisible()
    await expect(page.getByText('status_update').first()).toBeVisible()
  })
})
