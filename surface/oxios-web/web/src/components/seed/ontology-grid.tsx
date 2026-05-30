import { Box } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import type { SeedEntity } from '@/types/seed'

export function OntologyGrid({ entities }: { entities?: SeedEntity[] }) {
  const { t } = useTranslation()
  if (!entities?.length) return null

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Box className="h-4 w-4" /> {t('seeds.ontology')}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="grid gap-3 md:grid-cols-2">
          {entities.map((entity) => (
            <div key={entity.name} className="flex items-start gap-2 rounded-lg border p-3">
              <Badge variant="outline" className="mt-0.5 text-xs">
                {entity.kind}
              </Badge>
              <div>
                <p className="text-sm font-medium">{entity.name}</p>
                {entity.description && (
                  <p className="text-xs text-muted-foreground">{entity.description}</p>
                )}
              </div>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  )
}
