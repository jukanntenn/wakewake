'use client'

import { useTranslations } from 'next-intl'
import { useTheme } from 'next-themes'
import { Menu } from '@/components/ui/menu'
import { Button } from '@/components/ui/button'
import { Sun, Moon, Monitor } from 'lucide-react'

export function ThemeToggle() {
  const { theme, setTheme } = useTheme()
  const t = useTranslations('theme')

  const getThemeIcon = () => {
    switch (theme) {
      case 'light':
        return <Sun className="h-4 w-4" />
      case 'dark':
        return <Moon className="h-4 w-4" />
      default:
        return <Monitor className="h-4 w-4" />
    }
  }

  return (
    <div data-testid="theme-toggle-wrapper">
      <Menu.Root>
        <Menu.Trigger render={<Button variant="ghost" size="icon" />}>
          {getThemeIcon()}
          <span className="sr-only">{t('toggleTheme')}</span>
        </Menu.Trigger>
        <Menu.Popup>
          <Menu.RadioGroup value={theme} onValueChange={setTheme}>
            <Menu.RadioItem value="light" data-testid="theme-light">
              <Sun className="h-4 w-4" />
              {t('light')}
            </Menu.RadioItem>
            <Menu.RadioItem value="dark" data-testid="theme-dark">
              <Moon className="h-4 w-4" />
              {t('dark')}
            </Menu.RadioItem>
            <Menu.RadioItem value="system" data-testid="theme-system">
              <Monitor className="h-4 w-4" />
              {t('system')}
            </Menu.RadioItem>
          </Menu.RadioGroup>
        </Menu.Popup>
      </Menu.Root>
    </div>
  )
}
