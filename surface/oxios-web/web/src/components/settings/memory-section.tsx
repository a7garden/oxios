import { Brain, Database, Moon, Sparkles } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import type { SettingsFieldDef } from './field-defs'
import { FieldRow } from './field-row'

interface MemorySectionProps {
  /** Field defs for the memory section, grouped by sub-section id. */
  fieldsBySubsection: Record<string, SettingsFieldDef[]>
  /** Current form values, keyed by section.dotted.field, e.g. `memory.sqlite.path`. */
  formValues: Record<string, Record<string, string | boolean | string[]>>
  onFieldChange: (sectionKey: string, fieldKey: string, value: string | boolean | string[]) => void
}

const SUBSECTIONS: { id: string; titleKey: string; icon: React.ReactNode }[] = [
  { id: 'storage', titleKey: 'settings.memoryStorage', icon: <Database className="h-4 w-4" /> },
  { id: 'embedding', titleKey: 'settings.memoryEmbedding', icon: <Sparkles className="h-4 w-4" /> },
  { id: 'learning', titleKey: 'settings.memoryLearning', icon: <Brain className="h-4 w-4" /> },
  { id: 'dream', titleKey: 'settings.memoryDream', icon: <Moon className="h-4 w-4" /> },
]

/**
 * Memory settings rendered as 4 sub-cards: Storage / Embedding /
 * Learning / Dream. Each sub-section is collapsible-friendly (just a
 * header + field rows for now).
 */
export function MemorySection({
  fieldsBySubsection,
  formValues,
  onFieldChange,
}: MemorySectionProps) {
  const { t } = useTranslation()

  return (
    <div className="space-y-4">
      {SUBSECTIONS.map((sub) => {
        const fields = fieldsBySubsection[sub.id] ?? []
        if (fields.length === 0) return null
        return (
          <Card key={sub.id}>
            <CardHeader className="pb-4">
              <CardTitle className="flex items-center gap-2 text-base">
                {sub.icon}
                {t(sub.titleKey)}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              {fields.map((field, i) => (
                <div key={field.key}>
                  {i > 0 && <Separator className="mb-4" />}
                  <FieldRow
                    sectionKey="memory"
                    field={field}
                    value={
                      formValues.memory?.[field.key] as
                        | string
                        | boolean
                        | string[]
                        | number
                        | undefined
                    }
                    onChange={(val) => onFieldChange('memory', field.key, val)}
                    sectionValues={formValues.memory}
                  />
                </div>
              ))}
            </CardContent>
          </Card>
        )
      })}
    </div>
  )
}
