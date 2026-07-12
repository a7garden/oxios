import { Pencil } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { CronScheduleEditor } from '@/components/cron/cron-schedule-editor'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import type { CronJob } from '@/types'

export interface CronJobPatch {
  name: string
  schedule: string
  goal: string
}

interface EditCronDialogProps {
  job: CronJob | null
  isPending: boolean
  onOpenChange: (open: boolean) => void
  onSave: (patch: CronJobPatch) => void
}

/**
 * Cron job 편집 다이얼로그. 백엔드 POST /api/cron-jobs/:id/edit 으로
 * name/schedule/goal/enabled 를 부분 업데이트합니다.
 */
export function EditCronDialog({ job, isPending, onOpenChange, onSave }: EditCronDialogProps) {
  const { t } = useTranslation()
  const [name, setName] = useState('')
  const [schedule, setSchedule] = useState('')
  const [goal, setGoal] = useState('')

  useEffect(() => {
    if (job) {
      setName(job.name)
      setSchedule(job.schedule)
      setGoal(job.goal)
    }
  }, [job])

  const close = () => onOpenChange(false)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!job) return
    const n = name.trim()
    const s = schedule.trim()
    const g = goal.trim()
    if (!n || !s || !g) return
    if (n === job.name && s === job.schedule && g === job.goal) {
      close()
      return
    }
    onSave({ name: n, schedule: s, goal: g })
  }

  return (
    <Dialog open={job !== null} onOpenChange={(o) => !o && close()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Pencil className="h-5 w-5" />
            {t('cronJobs.edit')}
          </DialogTitle>
          <DialogDescription>{t('cronJobs.editDescription')}</DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="cron-edit-name">{t('cronJobs.name')}</Label>
            <Input
              id="cron-edit-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              autoFocus
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="cron-edit-schedule">{t('cronJobs.schedule')}</Label>
            <CronScheduleEditor id="cron-edit-schedule" value={schedule} onChange={setSchedule} />
          </div>
          <div className="space-y-2">
            <Label htmlFor="cron-edit-goal">{t('cronJobs.goal')}</Label>
            <Input
              id="cron-edit-goal"
              value={goal}
              onChange={(e) => setGoal(e.target.value)}
              placeholder={t('cronJobs.goalPlaceholder')}
            />
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              {t('common.cancel')}
            </Button>
            <Button
              type="submit"
              disabled={!name.trim() || !schedule.trim() || !goal.trim() || isPending}
            >
              {isPending ? t('common.saving') : t('common.save')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
