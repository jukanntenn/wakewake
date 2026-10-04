'use client'

// ConfirmDialog：声明式确认弹窗（§5.3/§13.7/§11.A.5）。
// 基于 AlertDialog（强制模态）。内部管理 confirming/error 状态。
// onConfirm 必须可 async，组件 await 它：成功 → onClose；失败 → 设置 error 不关闭。
// focus 默认在 Cancel（安全默认）。

import * as React from 'react'
import { useTranslations } from 'next-intl'
import { Loader2, TriangleAlert, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  AlertDialog,
  AlertDialogClose,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'

export interface ConfirmDialogProps {
  open: boolean
  onConfirm: () => void | Promise<void>
  onClose: () => void
  variant?: 'default' | 'warning' | 'danger'
  title: string
  description: React.ReactNode
  confirmText?: string
  cancelText?: string
  children?: React.ReactNode
}

export function ConfirmDialog({
  open,
  onConfirm,
  onClose,
  variant = 'default',
  title,
  description,
  confirmText,
  cancelText,
  children,
}: ConfirmDialogProps) {
  const tCommon = useTranslations('common')
  const [confirming, setConfirming] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)

  // 对话框关闭时重置内部状态
  React.useEffect(() => {
    if (!open) {
      setConfirming(false)
      setError(null)
    }
  }, [open])

  const handleConfirm = async () => {
    setConfirming(true)
    setError(null)
    try {
      await onConfirm()
      onClose()
    } catch (e) {
      // 失败：Dialog 不关闭，显示错误。尝试从 ApiError 提取 message。
      const msg = e instanceof Error ? e.message : String(e)
      setError(msg)
      setConfirming(false)
    }
  }

  return (
    <AlertDialog open={open} onOpenChange={(o) => !o && onClose()}>
      <AlertDialogContent>
        <div className="flex items-start gap-3">
          {variant === 'danger' && (
            <TriangleAlert className="text-destructive mt-0.5 h-5 w-5 shrink-0" />
          )}
          {variant === 'warning' && (
            <TriangleAlert className="text-warning mt-0.5 h-5 w-5 shrink-0" />
          )}
          <div className="flex-1">
            <AlertDialogTitle>{title}</AlertDialogTitle>
            <AlertDialogDescription>{description}</AlertDialogDescription>
          </div>
          <AlertDialogClose className="text-ink-muted hover:text-ink" disabled={confirming}>
            <X className="h-4 w-4" />
          </AlertDialogClose>
        </div>

        {children}

        {error && <p className="text-destructive mt-3 text-sm">{error}</p>}

        <div className="mt-6 flex flex-col gap-2 sm:flex-row sm:justify-end">
          <AlertDialogClose
            disabled={confirming}
            render={<Button variant="outline">{cancelText ?? tCommon('cancel')}</Button>}
          />
          <Button
            variant={variant === 'danger' ? 'destructive' : 'default'}
            disabled={confirming}
            onClick={handleConfirm}
          >
            {confirming ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              (confirmText ?? tCommon('confirm'))
            )}
          </Button>
        </div>
      </AlertDialogContent>
    </AlertDialog>
  )
}
