import { vi } from 'vitest'

export const useTranslation = vi.fn(() => ({
  t: (key: string) => key,
  i18n: { language: 'en' },
}))

export const Trans = vi.fn(({ children }: { children: React.ReactNode }) => children)
export const initReactI18next = { type: '3rdParty' as const }
export const I18nextProvider = ({ children }: { children: React.ReactNode }) => children
