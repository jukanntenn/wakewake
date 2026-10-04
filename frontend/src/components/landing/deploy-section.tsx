'use client'

// § DEPLOY:终端卡(hairline 标题栏,不用 macOS 圆点)+ 三个事实 chip。
// 命令与 README Quick start 一字不差;✔ 行是体验示意,不含虚构数字。

import { useTranslations } from 'next-intl'
import { SectionHeading } from './section-heading'

export function DeploySection() {
  const t = useTranslations('landing.deploy')

  return (
    <section id="deploy" className="border-hairline bg-surface-1 scroll-mt-20 border-y">
      <div className="mx-auto max-w-[1200px] px-4 py-16 md:px-6 md:py-20">
        <SectionHeading ns="landing.deploy" />
        <p className="text-ink-muted mx-auto mt-4 max-w-2xl text-center text-sm leading-relaxed">
          {t('body')}
        </p>

        <div className="border-hairline bg-surface-2 mx-auto mt-10 max-w-2xl rounded-lg border md:mt-12">
          <div className="border-hairline flex items-center justify-between border-b px-4 py-2.5">
            <span className="text-ink-muted font-mono text-xs">{t('terminalLabel')}</span>
            <span className="text-ink-subtle font-mono text-xs">sh</span>
          </div>
          <div className="space-y-1.5 p-4 font-mono text-xs leading-relaxed">
            <p>
              <span className="text-ink-subtle">$ </span>
              <span className="text-ink">cp docker/config.example.toml docker/config.toml</span>
            </p>
            <p>
              <span className="text-ink-subtle">$ </span>
              <span className="text-ink">cp docker/.env.example docker/.env</span>
            </p>
            <p className="text-ink-subtle"> # 3 secrets → openssl rand -base64 32</p>
            <p>
              <span className="text-ink-subtle">$ </span>
              <span className="text-ink">docker compose -f docker/docker-compose.yml up -d</span>
            </p>
            <p className="text-ink-muted pt-2">✔ listening · http://&lt;host&gt;:8443</p>
            <p className="text-ink-muted">✔ migrations applied · bootstrap admin ready</p>
          </div>
        </div>

        <div className="mt-6 flex flex-wrap items-center justify-center gap-2">
          {(['chip1', 'chip2', 'chip3'] as const).map((key) => (
            <span
              key={key}
              className="bg-surface-2 border-hairline text-ink-muted rounded-full border px-3 py-1 font-mono text-xs"
            >
              {t(key)}
            </span>
          ))}
        </div>
      </div>
    </section>
  )
}
