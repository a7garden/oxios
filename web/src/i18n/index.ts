import i18n from 'i18next'
import LanguageDetector from 'i18next-browser-languagedetector'
import { initReactI18next } from 'react-i18next'
import en from './locales/en.json'
import ko from './locales/ko.json'

const resources = {
  en: { common: en },
  ko: { common: ko },
}

export const initI18n = () => {
  i18n
    .use(LanguageDetector)
    .use(initReactI18next)
    .init({
      resources,
      fallbackLng: 'en',
      supportedLngs: ['en', 'ko'],
      defaultNS: 'common',
      ns: ['common'],
      interpolation: {
        escapeValue: false,
      },
      detection: {
        order: ['localStorage', 'navigator'],
        caches: ['localStorage'],
      },
    })

  return i18n
}

export default i18n
