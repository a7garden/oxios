import i18next from 'i18next'
import type { ReactElement } from 'react'
import { I18nextProvider, initReactI18next } from 'react-i18next'

import en from '../src/i18n/locales/en.json'

/**
 * A standalone, pre-initialized i18next instance bound to the `en`
 * locale. Mirrors the namespace layout used by `src/i18n/index.ts`:
 *
 *   resources.en.common = en.json   (defaultNS: 'common')
 *
 * Resources are inlined, so the store is populated synchronously at
 * module load — no HTTP backend, no `LanguageDetector`. This is the
 * lightest way to drive `useTranslation()` inside Storybook stories.
 */
const mockI18n = i18next.createInstance()

mockI18n.use(initReactI18next).init({
  resources: { en: { common: en } },
  lng: 'en',
  fallbackLng: 'en',
  defaultNS: 'common',
  ns: ['common'],
  interpolation: { escapeValue: false },
})

export { mockI18n }

/**
 * Storybook decorator that wraps a story in an
 * {@link I18nextProvider} bound to the shared `mockI18n` instance.
 *
 * Apply it to any component that calls `useTranslation()` from
 * `react-i18next`:
 *
 * ```ts
 * export const MyStory: Story = {
 *   decorators: [i18nDecorator],
 *   render: () => <MyComponent />,
 * }
 * ```
 *
 * We deliberately do NOT register this as a global decorator in
 * `preview.tsx` — pure-ui stories (Button, Card, …) don't need the
 * i18n provider and should stay untouched.
 */
export const i18nDecorator = (Story: () => ReactElement): ReactElement => (
  <I18nextProvider i18n={mockI18n}>{Story()}</I18nextProvider>
)
