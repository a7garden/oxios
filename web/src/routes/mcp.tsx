import { createFileRoute } from '@tanstack/react-router'
import { Plus, Server, Terminal, Wrench } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { AddServerDialog } from '@/components/mcp/add-server-dialog'
import { ServerList } from '@/components/mcp/server-list'
import { ToolList } from '@/components/mcp/tool-list'
import { ToolTester } from '@/components/mcp/tool-tester'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

export const Route = createFileRoute('/mcp')({ component: McpPage })

function McpPage() {
  const { t } = useTranslation()
  const [addDialogOpen, setAddDialogOpen] = useState(false)

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('mcp.title', 'MCP Servers')}</h1>
          <p className="text-muted-foreground">
            {t('mcp.subtitle', 'Manage Model Context Protocol servers and tools')}
          </p>
        </div>
        <Button onClick={() => setAddDialogOpen(true)}>
          <Plus className="h-4 w-4 mr-1" /> {t('mcp.addServer', 'Add Server')}
        </Button>
      </div>

      <Tabs defaultValue="servers">
        <TabsList>
          <TabsTrigger value="servers" className="flex items-center gap-1.5">
            <Server className="h-4 w-4" /> {t('mcp.servers', 'Servers')}
          </TabsTrigger>
          <TabsTrigger value="tools" className="flex items-center gap-1.5">
            <Wrench className="h-4 w-4" /> {t('mcp.tools', 'Tools')}
          </TabsTrigger>
          <TabsTrigger value="test" className="flex items-center gap-1.5">
            <Terminal className="h-4 w-4" /> {t('mcp.test', 'Test')}
          </TabsTrigger>
        </TabsList>

        <TabsContent value="servers">
          <Card>
            <CardContent className="pt-6">
              <ServerList />
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="tools">
          <Card>
            <CardContent className="pt-6">
              <ToolList />
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="test">
          <Card>
            <CardContent className="pt-6">
              <ToolTester />
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>

      <AddServerDialog open={addDialogOpen} onOpenChange={setAddDialogOpen} />
    </div>
  )
}
