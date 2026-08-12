'use client'

// Agents 页面：状态卡 + 公钥展示 + 配对指引。
// pairing code：pending 状态完整可见；online/offline 脱敏；rotate 后短暂显示完整新码。
// 公钥：标准 multi-line PEM 按行展示（已知服务如 SSH/Vercel deploy keys 均按行渲染）。

import { useEffect, useState } from 'react'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { RefreshCw, Copy, Check } from 'lucide-react'
import { useDefaultAgent, useRotatePairingCode } from '@/hooks/useAgents'
import { copyText } from '@/lib/clipboard'
import { BrandMark } from '@/components/brand/brand-logo'
import { RelativeTime } from '@/components/ui/relative-time'

export default function AgentsPage() {
  const t = useTranslations('agent')
  const ta = useTranslations('agents')
  const { data: agent, isLoading } = useDefaultAgent()
  const rotateMut = useRotatePairingCode()
  const [copied, setCopied] = useState(false)
  // L2 两段式按钮 armed 态（§11.B.6）：第一次点击 armed，3s 内再点执行，超时/移出恢复。
  const [armed, setArmed] = useState(false)
  const [rotateTimer, setRotateTimer] = useState<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    if (!copied) return
    const id = setTimeout(() => setCopied(false), 2000)
    return () => clearTimeout(id)
  }, [copied])

  // 清理 rotate timer on unmount
  useEffect(() => {
    return () => {
      if (rotateTimer) clearTimeout(rotateTimer)
    }
  }, [rotateTimer])

  const disarmRotate = () => {
    if (rotateTimer) clearTimeout(rotateTimer)
    setRotateTimer(null)
    setArmed(false)
  }

  const onRotateClick = async () => {
    if (!armed) {
      // 第一次点击：armed，3s 后自动解除
      setArmed(true)
      const timer = setTimeout(() => {
        setArmed(false)
        setRotateTimer(null)
      }, 3000)
      setRotateTimer(timer)
      return
    }
    // armed，第二次点击 → 执行
    disarmRotate()
    try {
      const resp = await rotateMut.mutateAsync()
      toast.success(t('rotateSuccess'))
      if (await copyText(resp.pairing_code)) {
        toast.message(t('copiedNewCode'))
      } else {
        toast.warning(t('codeCopiedManualHint'))
      }
    } catch {
      toast.error(t('rotateFailed'))
    }
  }

  const onCopy = async () => {
    if (!agent?.pairing_code) return
    if (await copyText(agent.pairing_code)) {
      setCopied(true)
      toast.success(t('copied'))
    } else {
      toast.error(t('copyFailed'))
    }
  }

  if (isLoading) {
    return (
      <div className="flex min-h-[50vh] items-center justify-center">
        <BrandMark size={32} className="animate-pulse" />
      </div>
    )
  }
  if (!agent) {
    return <p className="text-ink-muted">{t('status.pending')}</p>
  }

  const statusColor =
    agent.status === 'online'
      ? 'bg-success'
      : agent.status === 'offline'
        ? 'bg-ink-subtle'
        : 'bg-warning'

  // 脱敏码含 ****（后端 mask_code），不可用于实际连接。
  const isMasked = agent.pairing_code.includes('****')

  return (
    <div className="space-y-6">
      <h1 className="text-ink text-2xl font-semibold tracking-tight">{ta('title')}</h1>

      {/* 状态卡 */}
      <div className="border-hairline bg-surface-1 shadow-card rounded-lg border p-6">
        <div className="flex items-center gap-2">
          <span className={`h-2 w-2 rounded-full ${statusColor}`} />
          <span className="text-ink font-medium">{agent.name}</span>
          <span className="text-ink-muted text-sm">
            {t(`status.${agent.status}` as 'pending' | 'online' | 'offline')}
          </span>
        </div>

        {/* pairing code（pending/rotate 后完整；online/offline 脱敏） */}
        <div className="mt-4">
          <label className="text-ink-muted text-sm font-medium">{t('pairingCode')}</label>
          <div className="mt-1.5 flex items-center gap-2">
            <code className="border-hairline bg-surface-2 text-ink flex-1 overflow-x-auto rounded-md border px-3 py-2 font-mono text-sm">
              {agent.pairing_code}
            </code>
            <button
              onClick={onCopy}
              className="border-hairline text-ink-muted hover:bg-surface-2 hover:text-ink flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md border transition-colors"
              aria-label={t('copy')}
            >
              {copied ? <Check className="text-success h-4 w-4" /> : <Copy className="h-4 w-4" />}
            </button>
            <button
              onClick={onRotateClick}
              onMouseLeave={disarmRotate}
              disabled={rotateMut.isPending}
              className={
                armed
                  ? 'border-destructive text-destructive hover:bg-destructive/10 flex h-9 items-center gap-1.5 rounded-md border px-3 text-sm font-medium transition-colors'
                  : 'border-hairline text-ink-muted hover:bg-surface-2 hover:text-ink flex h-9 items-center gap-1.5 rounded-md border px-3 text-sm font-medium transition-colors'
              }
            >
              <RefreshCw className={`h-3.5 w-3.5 ${rotateMut.isPending ? 'animate-spin' : ''}`} />
              {armed ? t('confirmRotate') : t('rotate')}
            </button>
          </div>
        </div>

        {/* public key：标准 PEM 多行展示（OpenSSL/SSH 公钥惯例） */}
        {agent.public_key && (
          <div className="mt-4">
            <label className="text-ink-muted text-sm font-medium">{t('publicKey')}</label>
            <pre className="border-hairline bg-surface-2 text-ink-subtle mt-1.5 max-h-48 overflow-auto rounded-md border p-3 font-mono text-xs leading-relaxed">
              {agent.public_key}
            </pre>
          </div>
        )}

        {/* last_seen */}
        {agent.last_seen && (
          <p className="text-ink-subtle mt-4 text-xs">
            {t('lastSeen')}: <RelativeTime date={agent.last_seen} />
          </p>
        )}
      </div>

      {/* setup instructions */}
      <div className="border-hairline bg-surface-1 shadow-card rounded-lg border p-6">
        <h2 className="text-ink font-medium">{t('setupInstructions')}</h2>
        {isMasked ? (
          // 脱敏码不能用于实际连接——提示用户先 rotate 拿到完整码。
          <p className="text-ink-muted mt-2 text-sm">{t('maskedCodeHint')}</p>
        ) : (
          <pre className="border-hairline bg-surface-2 text-ink-subtle mt-2 overflow-auto rounded-md border p-3 font-mono text-xs leading-relaxed">
            {`# 1. 安装 wakewake-agent
# 2. 用 pairing code 连接（首连会自动上报公钥完成配对）
wakewake-agent \\
  --server https://wakewake.app \\
  --pairing-code ${agent.pairing_code}`}
          </pre>
        )}
      </div>
    </div>
  )
}
