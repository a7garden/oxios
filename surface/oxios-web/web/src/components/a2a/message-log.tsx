import { useTranslation } from 'react-i18next'
import type { A2AMessage } from '@/types/a2a'

interface Props {
  messages: A2AMessage[]
}

export function MessageLog({ messages }: Props) {
  const { t } = useTranslation()

  if (messages.length === 0) {
    return (
      <div className="flex items-center justify-center h-48 text-muted-foreground">
        {t('a2a.noMessages')}
      </div>
    )
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b text-left">
            <th className="pb-2 font-medium text-muted-foreground">{t('a2a.timestamp')}</th>
            <th className="pb-2 font-medium text-muted-foreground">{t('a2a.direction')}</th>
            <th className="pb-2 font-medium text-muted-foreground">{t('a2a.messageType')}</th>
            <th className="pb-2 font-medium text-muted-foreground">{t('a2a.status')}</th>
          </tr>
        </thead>
        <tbody>
          {messages.map((msg) => (
            <tr key={msg.request_id} className="border-b last:border-0 hover:bg-muted/50">
              <td className="py-2 font-mono text-xs">
                {new Date(msg.timestamp).toLocaleTimeString()}
              </td>
              <td className="py-2">
                <span className="font-medium">{msg.from_agent}</span>
                <span className="text-muted-foreground mx-1">→</span>
                <span className="font-medium">{msg.to_agent}</span>
              </td>
              <td className="py-2">
                <span className="rounded bg-muted px-1.5 py-0.5 text-xs font-mono">
                  {msg.message_type}
                </span>
              </td>
              <td className="py-2">
                {msg.accepted ? (
                  <span className="text-emerald-600">✅ Accepted</span>
                ) : (
                  <span className="text-amber-500">⏳ Pending</span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
