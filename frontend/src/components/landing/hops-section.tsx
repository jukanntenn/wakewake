'use client'

// § HOW IT WORKS:三跳横条放大。每跳一个 mono 证据物件(密文 chip / 终端 / hexdump),
// 桌面右侧配拓扑残影(对应边高亮)。03 WAKE 的 ● online 是全页绿色第三处。

import type { ReactNode } from 'react'
import { useTranslations } from 'next-intl'
import { SectionHeading } from './section-heading'

/** 拓扑残影:4 节点迷你图,activeEdge(0-2)高亮为 ink,其余 hairline。纯装饰。 */
function TopologyGhost({ activeEdge }: { activeEdge: 0 | 1 | 2 }) {
  const edges = [
    { x1: 46, x2: 68 },
    { x1: 114, x2: 136 },
    { x1: 182, x2: 204 },
  ]
  return (
    <svg viewBox="0 0 250 40" className="h-10 w-[250px]" aria-hidden="true">
      {edges.map(({ x1, x2 }, i) => (
        <line
          key={x1}
          x1={x1}
          y1="20"
          x2={x2}
          y2="20"
          className={i === activeEdge ? 'stroke-ink-muted' : 'stroke-hairline'}
          strokeWidth={i === activeEdge ? 1.5 : 1}
        />
      ))}
      {[0, 68, 136, 204].map((x) => (
        <rect
          key={x}
          x={x}
          y="6"
          width="46"
          height="28"
          rx="6"
          className="fill-surface-1 stroke-hairline"
        />
      ))}
    </svg>
  )
}

function HopRow({
  label,
  title,
  body,
  artifact,
  activeEdge,
}: {
  label: string
  title: string
  body: string
  artifact: ReactNode
  activeEdge: 0 | 1 | 2
}) {
  return (
    <div className="border-hairline grid gap-6 border-t py-10 first:border-t-0 first:pt-0 md:grid-cols-[1fr_auto] md:items-center md:gap-12">
      <div>
        <p className="text-ink-subtle font-mono text-xs tracking-widest uppercase">{label}</p>
        <h3 className="text-ink mt-2 text-lg font-semibold tracking-tight">{title}</h3>
        <p className="text-ink-muted mt-2 max-w-xl text-sm leading-relaxed">{body}</p>
        <div className="mt-4">{artifact}</div>
      </div>
      <div className="hidden md:block">
        <TopologyGhost activeEdge={activeEdge} />
      </div>
    </div>
  )
}

export function HopsSection() {
  const t = useTranslations('landing.hops')

  return (
    <section id="how-it-works" className="border-hairline bg-surface-1 scroll-mt-20 border-y">
      <div className="mx-auto max-w-[1200px] px-4 py-16 md:px-6 md:py-20">
        <SectionHeading ns="landing.hops" />

        <div className="mt-10 md:mt-14">
          <HopRow
            label="01 encrypt"
            activeEdge={0}
            title={t('hop1.title')}
            body={t('hop1.body')}
            artifact={
              <p className="bg-surface-2 text-ink-muted rounded-sm px-3 py-2 font-mono text-[11px] whitespace-nowrap">
                RSA-OAEP( 6C:4B:90:…:9E ) → 0x9f3a 7c12 … 44e1
                <span className="text-ink-subtle"> · server sees: ∅</span>
              </p>
            }
          />
          <HopRow
            label="02 relay"
            activeEdge={1}
            title={t('hop2.title')}
            body={t('hop2.body')}
            artifact={
              <div className="bg-surface-2 rounded-md px-3 py-2.5 font-mono text-[11px] leading-relaxed">
                <p className="text-ink-subtle">
                  <span className="text-ink-muted">$</span> wakewake-agent --server
                  https://wakewake.example.com \
                </p>
                <p className="text-ink-subtle"> --pairing 7F3K-A2BM-9Q4D</p>
                <p className="text-ink-muted">
                  ✔ paired · rsa-2048 keypair written · dialing out …
                </p>
              </div>
            }
          />
          <HopRow
            label="03 wake"
            activeEdge={2}
            title={t('hop3.title')}
            body={t('hop3.body')}
            artifact={
              <div className="bg-surface-2 rounded-md px-3 py-2.5 font-mono text-[11px] leading-relaxed">
                <p className="text-ink-muted">
                  FF FF FF FF FF FF · 6C 4B 90 … 9E · ×16 → 192.168.1.255:9
                </p>
                <p className="mt-1 flex items-center gap-2">
                  <span className="bg-success inline-block h-2 w-2 flex-shrink-0 rounded-full" />
                  <span className="text-ink">nas-01 online</span>
                </p>
              </div>
            }
          />
        </div>
      </div>
    </section>
  )
}
