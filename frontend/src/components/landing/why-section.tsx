'use client'

// § WHY:收益卡片 grid(8 张,lucide 图标)。
// 回答访客"为什么选它而不是商业远程开机/开机棒"——技术规格(自动迁移/PoW/JWT 等)
// 不在此层出现;每张卡都是用户可感知的收益,与 hero(一句话)/trust(威胁模型深挖)分层不重复。

import { FolderTree, History, Mic, Package, Router, Scale, Server, ShieldCheck } from 'lucide-react'
import { useTranslations } from 'next-intl'
import { SectionHeading } from './section-heading'

const CARDS = [
  { icon: Router, key: 'item1' as const },
  { icon: ShieldCheck, key: 'item2' as const },
  { icon: Server, key: 'item3' as const },
  { icon: Package, key: 'item4' as const },
  { icon: History, key: 'item5' as const },
  { icon: FolderTree, key: 'item6' as const },
  { icon: Mic, key: 'item7' as const },
  { icon: Scale, key: 'item8' as const },
]

export function WhySection() {
  const t = useTranslations('landing.why')

  return (
    <section id="why" className="scroll-mt-20">
      <div className="mx-auto max-w-[1200px] px-4 py-16 md:px-6 md:py-20">
        <SectionHeading ns="landing.why" />

        <div className="mx-auto mt-10 grid max-w-5xl gap-4 sm:grid-cols-2 md:mt-14 lg:grid-cols-4">
          {CARDS.map(({ icon: Icon, key }) => (
            <div
              key={key}
              className="border-hairline bg-surface-1 shadow-card rounded-lg border p-5"
            >
              <span className="border-hairline bg-surface-2 text-ink flex h-9 w-9 items-center justify-center rounded-md border">
                <Icon className="h-4 w-4" />
              </span>
              <h3 className="text-ink mt-3 text-[15px] leading-snug font-semibold">
                {t(`${key}.title`)}
              </h3>
              <p className="text-ink-muted mt-1.5 text-sm leading-relaxed">{t(`${key}.body`)}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}
