'use client'

// StatusIndicator：纯视觉原子（颜色 + 可选图标），不含业务语义。
// 所有规范绑定徽标基于它（§4.2/§12.7）。圆点 h-2 w-2 rounded-full；带图标 h-3.5 w-3.5。

import * as React from 'react'
import { cn } from '@/lib/utils'

export type StatusColor = 'success' | 'active' | 'warning' | 'error' | 'neutral'

// 颜色 token 映射（§4.2）：
// success → bg-success（绿）；active → bg-warning（琥珀，可脉冲）；warning → text-warning（图标用）；
// error → bg-destructive（红）；neutral → bg-ink-subtle（灰）。
// 注：active 与 warning 色相相同（琥珀），active 用于"进行中"圆点（bg），warning 用于三角图标（text）。
const DOT_BG: Record<StatusColor, string> = {
  success: 'bg-success',
  active: 'bg-warning',
  warning: 'bg-warning',
  error: 'bg-destructive',
  neutral: 'bg-ink-subtle',
}

// 图标色（带 icon 时用 text 色）：warning 用 text-warning（三角），其余沿用 bg 对应的 text 语义。
const ICON_TEXT: Record<StatusColor, string> = {
  success: 'text-success',
  active: 'text-warning',
  warning: 'text-warning',
  error: 'text-destructive',
  neutral: 'text-ink-subtle',
}

export interface StatusIndicatorProps {
  color: StatusColor
  icon?: React.ReactElement
  pulse?: boolean
  'aria-label'?: string
}

export function StatusIndicator({
  color,
  icon,
  pulse,
  'aria-label': ariaLabel,
}: StatusIndicatorProps) {
  // 纯圆点（无 icon）
  if (!icon) {
    return (
      <span
        role={ariaLabel ? 'img' : undefined}
        aria-label={ariaLabel}
        className={cn('inline-block h-2 w-2 rounded-full', DOT_BG[color], pulse && 'animate-pulse')}
      />
    )
  }
  // 带 icon：克隆传入的 lucide 图标，注入尺寸 + 色。
  return (
    <span className={cn('inline-flex', pulse && 'animate-pulse')}>
      {React.cloneElement(icon as React.ReactElement<{ className?: string }>, {
        className: cn('h-3.5 w-3.5', ICON_TEXT[color]),
      })}
    </span>
  )
}
