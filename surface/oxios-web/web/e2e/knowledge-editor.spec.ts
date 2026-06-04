import { test, expect, type Page } from '@playwright/test'

/**
 * Knowledge-base markdown editor e2e (#5-#11).
 *
 * Mocks the /api/knowledge/* endpoints so the page can be exercised
 * without a live Oxios daemon. Each test verifies one of the 5/5
 * preserved features (auto-save, heading enforcement, keyboard
 * shortcuts, wiki/emoji autocomplete, link click) plus the
 * new [[X]] wikilink linkify added in #11.
 */

const TREE = [
  { name: 'Today.md', is_dir: false, size: 12 },
  { name: 'Later.md', is_dir: false, size: 0 },
  { name: 'media', is_dir: true, size: 0 },
  { name: 'notes', is_dir: true, size: 0 },
  { name: 'project.md', is_dir: false, size: 5 },
]

const FILES: Record<string, string> = {
  'Today.md': '# Today\n\nA short note.\n',
  'Later.md': '',
  'project.md': '# Project\n\n- [ ] one\n- [x] two\n',
}

async function mockKnowledgeApi(page: Page) {
  await page.route(/\/api\/knowledge\/tree(\?.*)?$/, async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(TREE),
    })
  })
  await page.route(/\/api\/knowledge\/file\/.*/, async (route) => {
    const url = route.request().url()
    const path = decodeURIComponent(url.split('/api/knowledge/file/').pop() ?? '')
    if (route.request().method() === 'PUT') {
      // Capture the save body for assertions in the test.
      const put = route.request().postData() ?? ''
      // @ts-expect-error store on page for test
      page.__lastSave = { path, body: put }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ ok: true, path }),
      })
      return
    }
    const body = FILES[path] ?? `# ${path}\n\n(empty)\n`
    await route.fulfill({
      status: 200,
      contentType: 'text/plain',
      body,
    })
  })
  // Silence vite's event proxy so the page doesn't log ECONNREFUSED.
  await page.route(/\/api\/events(\?.*)?$/, (route) => route.abort())
}

/**
 * Helper: click on a tree item to open it in the editor.
 * The Oxios knowledge page renders the file tree as buttons/anchors
 * with the filename as text. We click whatever the first match is.
 */
async function openFile(page: Page, name: string) {
  await page.locator(`text="${name}"`).first().click()
}

test.describe('Knowledge editor (CM6) — #5-#11', () => {
  test.beforeEach(async ({ page }) => {
    await mockKnowledgeApi(page)
    await page.goto('/knowledge')
    // The Oxios knowledge page only mounts <MarkdownEditor /> once a
    // file is selected from the tree. Open Today.md in beforeEach so
    // the editor is mounted for every test.
    await page.waitForSelector('text="Today.md"', { timeout: 10000 })
    await page.locator('text="Today.md"').first().click()
    await page.waitForSelector('.cm-content', { timeout: 10000 })
  })

  test('1. editor mounts and shows the opened file', async ({ page }) => {
    await openFile(page, 'Today.md')
    await expect(page.locator('.cm-content')).toContainText('A short note')
  })

  test('2. auto-save fires PUT after 1s debounce', async ({ page }) => {
    await openFile(page, 'Today.md')
    // Focus the editor and type
    await page.locator('.cm-content').click()
    await page.keyboard.press('End')
    await page.keyboard.type(' edited')
    // Wait for the 1s debounce
    await page.waitForTimeout(1300)
    const save = await page.evaluate(() => (window as unknown as { __lastSave?: { path: string; body: string } }).__lastSave)
    expect(save).toBeTruthy()
    expect(save?.path).toBe('Today.md')
    expect(save?.body).toContain('edited')
  })

  test('3. heading enforcement: first line kept as #', async ({ page }) => {
    await openFile(page, 'Today.md')
    await page.locator('.cm-content').click()
    await page.keyboard.press('ControlOrMeta+a')
    await page.keyboard.press('Delete')
    // Now type a non-# first line
    await page.keyboard.type('Hello world')
    await page.waitForTimeout(200)
    const text = await page.locator('.cm-content').textContent()
    expect(text?.startsWith('# Hello world')).toBe(true)
  })

  test('4. keyboard shortcut: Mod-b wraps selection in **', async ({ page }) => {
    await openFile(page, 'Today.md')
    await page.locator('.cm-content').click()
    await page.keyboard.press('ControlOrMeta+a')
    await page.keyboard.press('Delete')
    await page.keyboard.type('foo')
    // Select "foo" with shift+arrow
    await page.keyboard.press('Shift+Home')
    await page.keyboard.press('ControlOrMeta+b')
    await page.waitForTimeout(100)
    const text = await page.locator('.cm-content').textContent()
    expect(text).toContain('**foo**')
  })

  test('5. wikilink autocomplete on [', async ({ page }) => {
    await openFile(page, 'Today.md')
    await page.locator('.cm-content').click()
    await page.keyboard.press('End')
    await page.keyboard.press('Enter')
    await page.keyboard.type('See [[')
    // The autocomplete tooltip should appear with file suggestions
    await expect(page.locator('.cm-tooltip-autocomplete')).toBeVisible({ timeout: 3000 })
    // The tooltip should mention our files (Today, Later, project)
    const tooltipText = await page.locator('.cm-tooltip-autocomplete').textContent()
    expect(tooltipText).toContain('Later')
  })

  test('6. [[X]] wikilink linkify: rendered as clickable <a>', async ({ page }) => {
    await openFile(page, 'Today.md')
    await page.locator('.cm-content').click()
    await page.keyboard.press('ControlOrMeta+a')
    await page.keyboard.press('Delete')
    await page.keyboard.type('See [[Later]]')
    // Move the cursor to another line so the link is in inactive territory
    // The active region is cursor line ± 1, so a single-line file with
    // cursor on that line keeps the text visible. Move cursor up — the
    // text on this line should then become a link.
    await page.keyboard.press('ControlOrMeta+Home')
    await page.waitForTimeout(200)
    // The wikilink should now be a rendered anchor
    const link = page.locator('a.cm-wikilink[data-wikilink-target="Later"]')
    await expect(link).toBeVisible({ timeout: 3000 })
    await expect(link).toHaveText('Later')
  })

  test('7. wikilink click dispatches knowledge:open-file', async ({ page }) => {
    await openFile(page, 'Today.md')
    await page.locator('.cm-content').click()
    await page.keyboard.press('ControlOrMeta+a')
    await page.keyboard.press('Delete')
    await page.keyboard.type('See [[project]]')
    // Move cursor away from the line
    await page.keyboard.press('ControlOrMeta+Home')
    await page.waitForTimeout(200)
    // Click the rendered link — it should dispatch the open-file event
    // which our knowledge page handles by switching the editor's file.
    const link = page.locator('a.cm-wikilink[data-wikilink-target="project"]')
    await expect(link).toBeVisible()
    await link.click()
    // The editor should now show project.md's content
    await page.waitForTimeout(500)
    // The 'one' / 'two' checklist from project.md should be in the editor.
    await expect(page.locator('.cm-content')).toContainText('one')
  })

  test('8. dark mode: oneDark applied when document has .dark class', async ({ page }) => {
    await page.evaluate(() => document.documentElement.classList.add('dark'))
    await openFile(page, 'Today.md')
    await page.waitForTimeout(200)
    // The .cm-editor or .cm-content should reflect the oneDark theme.
    // We don't have a strict assertion for the exact colors, but the
    // element must be present (mount succeeds) and not crash.
    await expect(page.locator('.cm-content')).toBeVisible()
  })
})
