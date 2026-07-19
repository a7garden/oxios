import { Moon, Sun, SunMoon } from 'lucide-react'
import { cn } from '@/lib/utils'
import { useThemeStore } from '@/stores/theme'

const ACCENT_COLORS = [
  { name: 'Blue', value: '217 91% 60%' },
  { name: 'Green', value: '142 71% 45%' },
  { name: 'Purple', value: '271 91% 65%' },
  { name: 'Orange', value: '25 95% 53%' },
  { name: 'Rose', value: '347 77% 50%' },
  { name: 'Teal', value: '173 80% 40%' },
  { name: 'Amber', value: '38 92% 50%' },
  { name: 'Slate', value: '215 14% 34%' },
]

const ACCENT_KEY = 'oxios-accent'

function loadAccent(): string {
  const saved = localStorage.getItem(ACCENT_KEY)
  if (saved) return saved
  return ACCENT_COLORS[0]!.value
}

function applyAccent(hsl: string) {
  document.documentElement.style.setProperty('--primary', hsl)
}

// Initialize on load
const initialAccent = loadAccent()
applyAccent(initialAccent)

export function AppearanceSettings({ className }: { className?: string }) {
  const { theme, setTheme } = useThemeStore()

  return (
    <div className={cn('space-y-6', className)}>
      {/* Theme mode */}
      <div>
        <label className="text-sm font-medium mb-2 block">Theme</label>
        <div className="grid grid-cols-3 gap-2">
          {(
            [
              ['light', Sun, 'Light'],
              ['dark', Moon, 'Dark'],
              ['system', SunMoon, 'System'],
            ] as const
          ).map(([mode, Icon, label]) => (
            <button
              key={mode}
              type="button"
              onClick={() => setTheme(mode)}
              className={cn(
                'flex flex-col items-center gap-1.5 rounded-lg border p-3 text-center transition-colors',
                theme === mode
                  ? 'border-primary bg-primary/5 text-primary'
                  : 'border-border hover:bg-muted/50 text-muted-foreground',
              )}
            >
              <Icon className="h-5 w-5" />
              <span className="text-xs font-medium">{label}</span>
            </button>
          ))}
        </div>
      </div>

      {/* Accent color */}
      <div>
        <label className="text-sm font-medium mb-2 block">Accent Color</label>
        <div className="flex gap-2 flex-wrap">
          {ACCENT_COLORS.map((c) => {
            const isActive = loadAccent() === c.value
            return (
              <button
                key={c.name}
                type="button"
                onClick={() => {
                  localStorage.setItem(ACCENT_KEY, c.value)
                  applyAccent(c.value)
                  // Force re-render via a small state update
                  window.dispatchEvent(new Event('accent-change'))
                }}
                title={c.name}
                className={cn(
                  'h-8 w-8 rounded-full border-2 transition-transform hover:scale-110',
                  isActive ? 'border-foreground scale-110' : 'border-transparent',
                )}
                style={{ backgroundColor: `hsl(${c.value})` }}
              />
            )
          })}
        </div>
      </div>
    </div>
  )
}
