'use client'

// Landing hero 拓扑图（DESIGN.md token + README 拓扑同构）。
// 桌面横版 / 移动纵版两个 SVG 变体;动画类 wk-packet-*/wk-dot 见 globals.css。
// 节点与边标签是技术词(mono 小写),不进 i18n;语义注解在 SVG 外走 i18n。

import { useTranslations } from 'next-intl'

const NODE_TITLE = 'fill-ink font-mono text-[13px]'
const NODE_SUB = 'fill-ink-muted font-mono text-[10px]'
const EDGE_LABEL = 'fill-ink-muted font-mono text-[10px]'
const EDGE_LINE = 'stroke-hairline'
const NODE_BOX = 'fill-surface-1 stroke-hairline'

export function TopologyDiagram() {
  const t = useTranslations('landing.hero')

  return (
    <div className="w-full">
      {/* 桌面横版 */}
      <svg
        viewBox="0 0 970 200"
        className="hidden w-full md:block"
        role="img"
        aria-label={t('diagramAlt')}
      >
        <defs>
          <marker
            id="wk-arrow-d"
            viewBox="0 0 8 8"
            markerWidth="7"
            markerHeight="7"
            refX="7"
            refY="4"
            orient="auto-start-reverse"
          >
            <path d="M0 0 L8 4 L0 8 z" className="fill-ink-muted" />
          </marker>
        </defs>

        {/* 基础边(hairline)+ 方向 */}
        <line
          x1="170"
          y1="100"
          x2="280"
          y2="100"
          className={EDGE_LINE}
          markerEnd="url(#wk-arrow-d)"
        />
        <line
          x1="430"
          y1="100"
          x2="540"
          y2="100"
          className={EDGE_LINE}
          markerStart="url(#wk-arrow-d)"
          markerEnd="url(#wk-arrow-d)"
        />
        <line
          x1="690"
          y1="100"
          x2="800"
          y2="100"
          className={EDGE_LINE}
          markerEnd="url(#wk-arrow-d)"
        />

        {/* 密文包流动(绿色,pathLength 归一化) */}
        <path d="M170 100 L280 100" pathLength={100} className="wk-packet wk-packet-1" />
        <path d="M430 100 L540 100" pathLength={100} className="wk-packet wk-packet-2" />
        <path d="M690 100 L800 100" pathLength={100} className="wk-packet wk-packet-3" />

        {/* 边标签 */}
        <text x="225" y="88" textAnchor="middle" className={EDGE_LABEL}>
          TLS
        </text>
        <text x="485" y="88" textAnchor="middle" className={EDGE_LABEL}>
          SSE · dials out
        </text>
        <text x="745" y="88" textAnchor="middle" className={EDGE_LABEL}>
          UDP · magic
        </text>

        {/* 节点 */}
        <g>
          <rect x="20" y="60" width="150" height="80" rx="12" className={NODE_BOX} />
          <text x="95" y="97" textAnchor="middle" className={NODE_TITLE}>
            browser
          </text>
          <text x="95" y="118" textAnchor="middle" className={NODE_SUB}>
            Web Crypto
          </text>
        </g>
        <g>
          <rect x="280" y="60" width="150" height="80" rx="12" className={NODE_BOX} />
          <text x="355" y="97" textAnchor="middle" className={NODE_TITLE}>
            2 GB VPS
          </text>
          <text x="355" y="118" textAnchor="middle" className={NODE_SUB}>
            caddy · rust
          </text>
        </g>
        <g>
          <rect x="540" y="60" width="150" height="80" rx="12" className={NODE_BOX} />
          <text x="615" y="97" textAnchor="middle" className={NODE_TITLE}>
            agent
          </text>
          <text x="615" y="118" textAnchor="middle" className={NODE_SUB}>
            rsa keypair
          </text>
        </g>
        <g>
          <rect x="800" y="60" width="150" height="80" rx="12" className={NODE_BOX} />
          <text x="875" y="97" textAnchor="middle" className={NODE_TITLE}>
            nas-01
          </text>
          <circle cx="875" cy="115" r="5" className="wk-dot" />
        </g>
      </svg>

      {/* 移动纵版 */}
      <svg
        viewBox="0 0 340 470"
        className="mx-auto w-full max-w-[340px] md:hidden"
        role="img"
        aria-label={t('diagramAlt')}
      >
        <defs>
          <marker
            id="wk-arrow-m"
            viewBox="0 0 8 8"
            markerWidth="7"
            markerHeight="7"
            refX="7"
            refY="4"
            orient="auto-start-reverse"
          >
            <path d="M0 0 L8 4 L0 8 z" className="fill-ink-muted" />
          </marker>
        </defs>

        <line
          x1="170"
          y1="85"
          x2="170"
          y2="135"
          className={EDGE_LINE}
          markerEnd="url(#wk-arrow-m)"
        />
        <line
          x1="170"
          y1="205"
          x2="170"
          y2="255"
          className={EDGE_LINE}
          markerStart="url(#wk-arrow-m)"
          markerEnd="url(#wk-arrow-m)"
        />
        <line
          x1="170"
          y1="325"
          x2="170"
          y2="375"
          className={EDGE_LINE}
          markerEnd="url(#wk-arrow-m)"
        />

        <path d="M170 85 L170 135" pathLength={100} className="wk-packet wk-packet-1" />
        <path d="M170 205 L170 255" pathLength={100} className="wk-packet wk-packet-2" />
        <path d="M170 325 L170 375" pathLength={100} className="wk-packet wk-packet-3" />

        <text x="180" y="113" className={EDGE_LABEL}>
          TLS
        </text>
        <text x="180" y="233" className={EDGE_LABEL}>
          SSE · dials out
        </text>
        <text x="180" y="353" className={EDGE_LABEL}>
          UDP · magic
        </text>

        <g>
          <rect x="95" y="15" width="150" height="70" rx="12" className={NODE_BOX} />
          <text x="170" y="45" textAnchor="middle" className={NODE_TITLE}>
            browser
          </text>
          <text x="170" y="66" textAnchor="middle" className={NODE_SUB}>
            Web Crypto
          </text>
        </g>
        <g>
          <rect x="95" y="135" width="150" height="70" rx="12" className={NODE_BOX} />
          <text x="170" y="165" textAnchor="middle" className={NODE_TITLE}>
            2 GB VPS
          </text>
          <text x="170" y="186" textAnchor="middle" className={NODE_SUB}>
            caddy · rust
          </text>
        </g>
        <g>
          <rect x="95" y="255" width="150" height="70" rx="12" className={NODE_BOX} />
          <text x="170" y="285" textAnchor="middle" className={NODE_TITLE}>
            agent
          </text>
          <text x="170" y="306" textAnchor="middle" className={NODE_SUB}>
            rsa keypair
          </text>
        </g>
        <g>
          <rect x="95" y="375" width="150" height="70" rx="12" className={NODE_BOX} />
          <text x="170" y="405" textAnchor="middle" className={NODE_TITLE}>
            nas-01
          </text>
          <circle cx="170" cy="424" r="5" className="wk-dot" />
        </g>
      </svg>

      {/* 语义注解:与节点对齐(桌面 4 列 / 移动 2 列) */}
      <div className="mx-auto mt-6 grid max-w-[970px] grid-cols-2 gap-x-4 gap-y-3 md:grid-cols-4">
        {[
          { key: 'noteBrowser', target: 'browser' },
          { key: 'noteServer', target: 'vps' },
          { key: 'noteAgent', target: 'agent' },
          { key: 'noteDevice', target: 'device' },
        ].map(({ key, target }) => (
          <div key={key} className="border-hairline border-t pt-2 text-center">
            <p className="text-ink-muted font-mono text-[10px] uppercase">{target}</p>
            <p className="text-ink-muted mt-1 text-xs">{t(key as 'noteBrowser')}</p>
          </div>
        ))}
      </div>
    </div>
  )
}
