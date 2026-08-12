'use client'

// AlertDialog：基于 Base UI AlertDialog 封装（§5.3/§12.9）。
// 强制模态（AlertDialog 固有，不可点外部关闭）。移动端靠近底部（bottom-4），桌面端居中（sm:top-1/2）。
// initialFocus 默认聚焦 Cancel（安全默认，§11.A.5）。

import * as React from 'react'
import { AlertDialog as AlertDialogPrimitive } from '@base-ui/react/alert-dialog'
import { cn } from '@/lib/utils'

function AlertDialog({ ...props }: React.ComponentProps<typeof AlertDialogPrimitive.Root>) {
  return <AlertDialogPrimitive.Root {...props} />
}

function AlertDialogTrigger({
  ...props
}: React.ComponentProps<typeof AlertDialogPrimitive.Trigger>) {
  return <AlertDialogPrimitive.Trigger {...props} />
}

function AlertDialogContent({
  className,
  children,
  ...props
}: React.ComponentProps<typeof AlertDialogPrimitive.Popup>) {
  return (
    <AlertDialogPrimitive.Portal>
      <AlertDialogPrimitive.Backdrop className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm" />
      <AlertDialogPrimitive.Popup
        initialFocus={undefined}
        className={cn(
          'bg-canvas fixed left-1/2 z-50 w-full -translate-x-1/2',
          'border-hairline shadow-modal bottom-4 rounded-lg border p-6',
          'sm:top-1/2 sm:-translate-y-1/2',
          'max-w-md',
          'data-[ending-style]:opacity-0 data-[starting-style]:opacity-0',
          className,
        )}
        {...props}
      >
        {children}
      </AlertDialogPrimitive.Popup>
    </AlertDialogPrimitive.Portal>
  )
}

function AlertDialogTitle({ ...props }: React.ComponentProps<typeof AlertDialogPrimitive.Title>) {
  return <AlertDialogPrimitive.Title className="text-ink text-lg font-semibold" {...props} />
}

function AlertDialogDescription({
  ...props
}: React.ComponentProps<typeof AlertDialogPrimitive.Description>) {
  return <AlertDialogPrimitive.Description className="text-ink-muted mt-2 text-sm" {...props} />
}

function AlertDialogClose({ ...props }: React.ComponentProps<typeof AlertDialogPrimitive.Close>) {
  return <AlertDialogPrimitive.Close {...props} />
}

export {
  AlertDialog,
  AlertDialogClose,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogTitle,
  AlertDialogTrigger,
}
