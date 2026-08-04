import { useEffect } from 'react'
import { codeApi } from '@/lib/code-api'
import { useCodeSessionStore } from '@/stores/code/code-session'

export function useCodeActions() {
  const { tabs, updateTab, session } = useCodeSessionStore()

  // Cmd+S saves all dirty tabs
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 's') {
        e.preventDefault()
        saveAll()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  })

  async function saveAll() {
    if (!session) return
    const dirty = tabs.filter((t) => t.isDirty)
    await Promise.all(
      dirty.map(async (tab) => {
        await codeApi.writeFile(tab.path, tab.content)
        updateTab(tab.id, { isDirty: false })
      }),
    )
  }

  return { saveAll }
}
