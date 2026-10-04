'use client'

// EmptyState：空状态（§4.7/§12.12）。图标 + 标题 + 描述 + 可选 action/link。

import * as React from 'react'

export interface EmptyStateProps {
  icon: React.ReactElement
  title: string
  description?: string
  action?: React.ReactNode
  link?: React.ReactNode
}

export function EmptyState({ icon, title, description, action, link }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center py-12 text-center">
      <div className="text-ink-subtle mb-4">
        {React.cloneElement(icon as React.ReactElement<{ className?: string }>, {
          className: 'h-12 w-12',
        })}
      </div>
      <h3 className="text-ink text-lg font-medium">{title}</h3>
      {description && <p className="text-ink-muted mt-1 max-w-sm text-sm">{description}</p>}
      {action && <div className="mt-4">{action}</div>}
      {link && <div className="mt-3">{link}</div>}
    </div>
  )
}
