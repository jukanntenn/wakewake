'use client'

// LanguageSwitcher：localStorage locale 模型（i18n.md §2 方案 B）。
// 切换语言不改 URL，仅更新 store + NextIntlClientProvider。

import { useLocaleContext } from '@/components/providers/LocaleProvider'
import { Select, SelectContent, SelectItem, SelectTrigger } from '@/components/ui/select'
import { Globe } from 'lucide-react'

const languages = [
  { code: 'en', name: 'English', flag: '🇺🇸' },
  { code: 'zh', name: '中文', flag: '🇨🇳' },
  { code: 'ja', name: '日本語', flag: '🇯🇵' },
  { code: 'ko', name: '한국어', flag: '🇰🇷' },
  { code: 'de', name: 'Deutsch', flag: '🇩🇪' },
  { code: 'fr', name: 'Français', flag: '🇫🇷' },
  { code: 'es', name: 'Español', flag: '🇪🇸' },
  { code: 'pt', name: 'Português', flag: '🇧🇷' },
] as const

export function LanguageSwitcher() {
  const { locale, setLocale } = useLocaleContext()

  const handleChange = (value: string | null) => {
    if (value) {
      setLocale(value as typeof locale)
    }
  }

  return (
    <div data-testid="language-switcher-wrapper">
      <Select value={locale} onValueChange={handleChange}>
        <SelectTrigger className="h-9 w-[40px] px-2">
          <Globe className="h-4 w-4" />
        </SelectTrigger>
        <SelectContent>
          {languages.map((lang) => (
            <SelectItem key={lang.code} value={lang.code} data-testid={`lang-${lang.code}`}>
              <span className="flex items-center gap-2">
                <span>{lang.flag}</span>
                <span>{lang.name}</span>
              </span>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  )
}
