import { useTranslation } from 'react-i18next'
import { Globe } from 'lucide-react'
import { Select } from '@/components/ui/select'

const languages = [
  { value: 'en', label: 'English' },
  { value: 'ko', label: '한국어' },
]

export function LanguageSelector() {
  const { i18n, t } = useTranslation()

  const handleChange = (lang: string) => {
    i18n.changeLanguage(lang)
    localStorage.setItem('i18nextLng', lang)
  }

  return (
    <div className="flex items-center gap-2">
      <Globe className="h-4 w-4 text-muted-foreground" />
      <Select
        value={i18n.language}
        onValueChange={handleChange}
        placeholder={t('common.language')}
        options={languages}
        className="w-[120px]"
      />
    </div>
  )
}