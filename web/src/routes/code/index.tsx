import { createFileRoute } from '@tanstack/react-router'
import { useTranslation } from 'react-i18next'
import { Code2, FolderOpen } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'

export const Route = createFileRoute('/code/')({
  component: CodeSessionPicker,
})

function CodeSessionPicker() {
  const { t } = useTranslation()
  return (
    <div className="flex h-full items-center justify-center p-8">
      <Card className="max-w-md w-full p-8">
        <div className="flex items-center gap-3 mb-6">
          <Code2 className="h-8 w-8 text-primary" />
          <div>
            <h1 className="text-xl font-bold">Code Workspace</h1>
            <p className="text-sm text-muted-foreground">AI-powered coding environment</p>
          </div>
        </div>
        <Button className="w-full" size="lg">
          <FolderOpen className="mr-2 h-4 w-4" />
          {t('code.openProject')}
        </Button>
      </Card>
    </div>
  )
}
