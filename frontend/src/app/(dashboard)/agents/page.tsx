'use client'

// Agents 页三态（specs/frontend/agent-onboarding.md）：pending 引导 / online 完成 / offline 修复。
// 启动命令由 lib/agent-command.ts 模板槽生成（--server 取当前站点 origin）；
// 状态由 useDefaultAgent() 5s 轮询驱动自动切换，无新增后端端点。

import { useEffect, useState, useSyncExternalStore } from 'react'
import Link from 'next/link'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { Check, Cloud, Copy, Home, Monitor, RefreshCw } from 'lucide-react'
import { useDefaultAgent, useRotatePairingCode } from '@/hooks/useAgents'
import { copyText } from '@/lib/clipboard'
import { buildAgentDockerCommand, buildAgentInstallCommand } from '@/lib/agent-command'
import { BrandMark } from '@/components/brand/brand-logo'
import { Button, buttonVariants } from '@/components/ui/button'
import { RelativeTime } from '@/components/ui/relative-time'
import { StatusIndicator } from '@/components/ui/status-indicator'
import { cn } from '@/lib/utils'

type LinkState = 'linked' | 'missing' | 'broken'
type CommandTab = 'linux' | 'docker'

const COMMAND_TABS: ReadonlyArray<{
  id: CommandTab
  labelKey: 'guide.tabLinux' | 'guide.tabDocker'
}> = [
  { id: 'linux', labelKey: 'guide.tabLinux' },
  { id: 'docker', labelKey: 'guide.tabDocker' },
]

const subscribeNoop = () => () => {}

// 当前站点 origin：useSyncExternalStore 的 server snapshot 为空串——
// 静态导出预渲染无 window，hydration 后自动取客户端值，无 mismatch。
function useSiteOrigin() {
  return useSyncExternalStore(
    subscribeNoop,
    () => window.location.origin,
    () => '',
  )
}

export default function AgentsPage() {
  const t = useTranslations('agent')
  const ta = useTranslations('agents')
  const { data: agent, isLoading } = useDefaultAgent()
  const rotateMut = useRotatePairingCode()
  const origin = useSiteOrigin()
  const [cmdTab, setCmdTab] = useState<CommandTab>('linux')
  const [cmdCopied, setCmdCopied] = useState(false)
  const [codeCopied, setCodeCopied] = useState(false)
  // rotate 两段式 armed 态（§11.B.6）：第一次点击 armed，3s 内再点执行，超时/移出恢复。
  const [armed, setArmed] = useState(false)
  const [rotateTimer, setRotateTimer] = useState<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    if (!cmdCopied) return
    const id = setTimeout(() => setCmdCopied(false), 2000)
    return () => clearTimeout(id)
  }, [cmdCopied])

  useEffect(() => {
    if (!codeCopied) return
    const id = setTimeout(() => setCodeCopied(false), 2000)
    return () => clearTimeout(id)
  }, [codeCopied])

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
      setArmed(true)
      const timer = setTimeout(() => {
        setArmed(false)
        setRotateTimer(null)
      }, 3000)
      setRotateTimer(timer)
      return
    }
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

  const onCopyCommand = async () => {
    if (!command) return
    if (await copyText(command)) {
      setCmdCopied(true)
      toast.success(t('copied'))
    } else {
      toast.error(t('copyFailed'))
    }
  }

  const onCopyCode = async () => {
    if (!agent?.pairing_code) return
    if (await copyText(agent.pairing_code)) {
      setCodeCopied(true)
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

  // 脱敏码含 ****（后端 mask_code），不可用于实际连接；rotate 是取回完整码的唯一途径。
  const isMasked = agent.pairing_code.includes('****')
  const command =
    !isMasked && origin
      ? cmdTab === 'linux'
        ? buildAgentInstallCommand(origin, agent.pairing_code)
        : buildAgentDockerCommand(origin, agent.pairing_code)
      : null

  const linkState: LinkState =
    agent.status === 'online' ? 'linked' : agent.status === 'offline' ? 'broken' : 'missing'

  return (
    <div className="space-y-6">
      {/* 状态条：名称 + 状态 + 最后在线 */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h1 className="text-ink text-2xl font-semibold tracking-tight">{ta('title')}</h1>
        <div className="text-ink-muted flex items-center gap-2 text-sm">
          <StatusIndicator
            color={
              agent.status === 'online'
                ? 'success'
                : agent.status === 'offline'
                  ? 'neutral'
                  : 'active'
            }
            pulse={agent.status === 'pending'}
          />
          <span className="text-ink font-medium">{agent.name}</span>
          <span>{t(`status.${agent.status}` as 'pending' | 'online' | 'offline')}</span>
          {agent.last_seen && (
            <span className="text-ink-subtle">
              · {t('lastSeen')} <RelativeTime date={agent.last_seen} />
            </span>
          )}
        </div>
      </div>

      {agent.status === 'online' ? (
        <OnlinePanel linkState={linkState} t={t} />
      ) : agent.status === 'offline' ? (
        <OfflinePanel
          t={t}
          repairHint={t('offline.repairHint')}
          rotateLabel={armed ? t('confirmRotate') : t('rotate')}
          armed={armed}
          rotatePending={rotateMut.isPending}
          onRotateClick={onRotateClick}
          onRotateLeave={disarmRotate}
        >
          {!isMasked && origin && (
            <CommandCard
              command={command}
              copied={cmdCopied}
              onCopy={onCopyCommand}
              copyLabel={t('guide.copyCommand')}
              tabs={COMMAND_TABS}
              activeTab={cmdTab}
              onTabChange={setCmdTab}
              t={t}
            />
          )}
        </OfflinePanel>
      ) : (
        <>
          <MentalModel linkState={linkState} t={t} />
          <section className="border-hairline bg-surface-1 shadow-card space-y-4 rounded-lg border p-6">
            <div>
              <h2 className="text-ink font-medium">{t('guide.whereTitle')}</h2>
              <p className="text-ink-muted mt-1 text-sm leading-relaxed">{t('guide.whereDesc')}</p>
            </div>
            <CommandCard
              command={command}
              copied={cmdCopied}
              onCopy={onCopyCommand}
              copyLabel={t('guide.copyCommand')}
              tabs={COMMAND_TABS}
              activeTab={cmdTab}
              onTabChange={setCmdTab}
              t={t}
            />
            <p className="text-ink-muted flex items-center gap-2 text-sm">
              <StatusIndicator color="active" pulse />
              {t('guide.waiting')}
            </p>
          </section>
        </>
      )}

      {/* 高级折叠：配对码 / 公钥 / 配置文件方式 */}
      <details className="border-hairline bg-surface-1 shadow-card rounded-lg border">
        <summary className="text-ink-muted hover:text-ink cursor-pointer px-5 py-3.5 text-sm font-medium select-none">
          {t('advanced')}
        </summary>
        <div className="border-hairline space-y-6 border-t px-5 py-5">
          <div>
            <label className="text-ink-muted text-sm font-medium">{t('pairingCode')}</label>
            <div className="mt-1.5 flex items-center gap-2">
              <code className="border-hairline bg-surface-2 text-ink flex-1 overflow-x-auto rounded-md border px-3 py-2 font-mono text-sm">
                {agent.pairing_code}
              </code>
              <button
                onClick={onCopyCode}
                className="border-hairline text-ink-muted hover:bg-surface-2 hover:text-ink flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md border transition-colors"
                aria-label={t('copy')}
              >
                {codeCopied ? (
                  <Check className="text-success h-4 w-4" />
                ) : (
                  <Copy className="h-4 w-4" />
                )}
              </button>
              <button
                onClick={onRotateClick}
                onMouseLeave={disarmRotate}
                disabled={rotateMut.isPending}
                className={cn(
                  'flex h-9 flex-shrink-0 items-center gap-1.5 rounded-md border px-3 text-sm font-medium transition-colors',
                  armed
                    ? 'border-destructive text-destructive hover:bg-destructive/10'
                    : 'border-hairline text-ink-muted hover:bg-surface-2 hover:text-ink',
                )}
              >
                <RefreshCw className={cn('h-3.5 w-3.5', rotateMut.isPending && 'animate-spin')} />
                {armed ? t('confirmRotate') : t('rotate')}
              </button>
            </div>
            {isMasked && <p className="text-ink-subtle mt-1.5 text-xs">{t('maskedCodeHint')}</p>}
          </div>

          <div>
            <label className="text-ink-muted text-sm font-medium">{t('publicKey')}</label>
            {agent.public_key ? (
              <pre className="border-hairline bg-surface-2 text-ink-subtle mt-1.5 max-h-48 overflow-auto rounded-md border p-3 font-mono text-xs leading-relaxed">
                {agent.public_key}
              </pre>
            ) : (
              <p className="text-ink-subtle mt-1.5 text-sm">{t('publicKeyNotAvailable')}</p>
            )}
          </div>

          <div>
            <h3 className="text-ink-muted text-sm font-medium">{t('advancedConfigTitle')}</h3>
            <p className="text-ink-muted mt-1 text-sm leading-relaxed">{t('advancedConfigDesc')}</p>
            <pre className="border-hairline bg-surface-2 text-ink-subtle mt-1.5 overflow-auto rounded-md border p-3 font-mono text-xs leading-relaxed">
              {`# ~/.wakewake/config.toml\nserver_url   = ${origin ? `"${origin}"` : '"<server_url>"'}\npairing_code = "${agent.pairing_code}"`}
            </pre>
          </div>

          <div>
            <h3 className="text-ink-muted text-sm font-medium">{t('service.title')}</h3>
            <p className="text-ink-muted mt-1 text-sm leading-relaxed">{t('service.desc')}</p>
            <pre className="border-hairline bg-surface-2 text-ink-subtle mt-1.5 overflow-x-auto rounded-md border p-3 font-mono text-xs leading-relaxed">
              sudo wakewake-agent service install
            </pre>
          </div>
        </div>
      </details>
    </div>
  )
}

function MentalModel({
  linkState,
  t,
}: {
  linkState: LinkState
  t: ReturnType<typeof useTranslations>
}) {
  return (
    <div className="border-hairline bg-surface-1 shadow-card rounded-lg border p-6">
      <div className="flex items-center gap-2 sm:gap-3">
        <ModelNode icon={<Cloud className="h-4 w-4" />} label={t('model.server')} />
        <Connector linkState={linkState} label={t(`model.${linkState}`)} />
        <ModelNode
          icon={<Home className="h-4 w-4" />}
          label={t('model.agent')}
          dimmed={linkState === 'missing'}
        />
        <Connector linkState="linked" label={t('model.linked')} />
        <ModelNode icon={<Monitor className="h-4 w-4" />} label={t('model.device')} />
      </div>
    </div>
  )
}

function ModelNode({
  icon,
  label,
  dimmed,
}: {
  icon: React.ReactElement
  label: string
  dimmed?: boolean
}) {
  return (
    <div className={cn('flex flex-col items-center gap-1.5', dimmed && 'opacity-50')}>
      <span className="border-hairline bg-surface-2 text-ink flex h-9 w-9 items-center justify-center rounded-md border">
        {icon}
      </span>
      <span className="text-ink-muted text-xs whitespace-nowrap">{label}</span>
    </div>
  )
}

function Connector({ linkState, label }: { linkState: LinkState; label: string }) {
  const linked = linkState === 'linked'
  return (
    <div className="flex min-w-8 flex-1 flex-col items-center gap-1 px-1">
      <div
        className={cn(
          'w-full border-t',
          linked ? 'border-hairline' : 'border-warning border-dashed',
          linkState === 'broken' && 'border-ink-subtle',
        )}
      />
      <span
        className={cn(
          'text-[11px] whitespace-nowrap',
          linked ? 'text-ink-subtle' : linkState === 'broken' ? 'text-ink-subtle' : 'text-warning',
        )}
      >
        {label}
      </span>
    </div>
  )
}

function CommandCard({
  command,
  copied,
  onCopy,
  copyLabel,
  tabs,
  activeTab,
  onTabChange,
  t,
}: {
  command: string | null
  copied: boolean
  onCopy: () => void
  copyLabel: string
  tabs: ReadonlyArray<{ id: CommandTab; labelKey: 'guide.tabLinux' | 'guide.tabDocker' }>
  activeTab: CommandTab
  onTabChange: (id: CommandTab) => void
  t: ReturnType<typeof useTranslations>
}) {
  return (
    <div className="border-hairline bg-surface-2 overflow-hidden rounded-lg border">
      <div className="border-hairline flex items-center justify-between gap-2 border-b px-4 py-2">
        <div
          className="flex items-center gap-1"
          role="tablist"
          aria-label={t('guide.commandLabel')}
        >
          {tabs.map((tab) => (
            <button
              key={tab.id}
              role="tab"
              aria-selected={activeTab === tab.id}
              onClick={() => onTabChange(tab.id)}
              className={cn(
                'rounded-md px-2.5 py-1 font-mono text-xs transition-colors',
                activeTab === tab.id ? 'bg-surface-3 text-ink' : 'text-ink-subtle hover:text-ink',
              )}
            >
              {t(tab.labelKey)}
            </button>
          ))}
        </div>
        <Button variant="secondary" size="sm" onClick={onCopy} disabled={!command}>
          {copied ? <Check className="text-success h-4 w-4" /> : <Copy className="h-4 w-4" />}
          {copyLabel}
        </Button>
      </div>
      <pre className="space-y-1.5 overflow-x-auto p-4 font-mono text-xs leading-relaxed">
        {command ? (
          command.split('\n').map((line) => (
            <span key={line} className="block">
              <span className="text-ink-subtle">$ </span>
              <span className="text-ink">{line}</span>
            </span>
          ))
        ) : (
          <span className="text-ink-subtle block">…</span>
        )}
      </pre>
    </div>
  )
}

function OnlinePanel({
  linkState,
  t,
}: {
  linkState: LinkState
  t: ReturnType<typeof useTranslations>
}) {
  return (
    <section className="border-hairline bg-surface-1 shadow-card space-y-5 rounded-lg border p-6">
      <MentalModel linkState={linkState} t={t} />
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-ink flex items-center gap-2 font-medium">
            <StatusIndicator color="success" />
            {t('online.title')}
          </h2>
          <p className="text-ink-muted mt-1 text-sm">{t('online.desc')}</p>
        </div>
        <Link href="/devices" className={buttonVariants()}>
          {t('online.cta')}
        </Link>
      </div>
    </section>
  )
}

function OfflinePanel({
  t,
  repairHint,
  rotateLabel,
  armed,
  rotatePending,
  onRotateClick,
  onRotateLeave,
  children,
}: {
  t: ReturnType<typeof useTranslations>
  repairHint: string
  rotateLabel: string
  armed: boolean
  rotatePending: boolean
  onRotateClick: () => void
  onRotateLeave: () => void
  children?: React.ReactNode
}) {
  return (
    <section className="border-hairline bg-surface-1 shadow-card space-y-4 rounded-lg border p-6">
      <div>
        <h2 className="text-ink font-medium">{t('offline.title')}</h2>
        <p className="text-ink-muted mt-1 text-sm leading-relaxed">{t('offline.desc')}</p>
      </div>
      <div className="flex flex-wrap items-center gap-3">
        <button
          onClick={onRotateClick}
          onMouseLeave={onRotateLeave}
          disabled={rotatePending}
          className={cn(
            'flex h-9 items-center gap-1.5 rounded-md border px-3 text-sm font-medium transition-colors',
            armed
              ? 'border-destructive text-destructive hover:bg-destructive/10'
              : 'border-hairline text-ink-muted hover:bg-surface-2 hover:text-ink',
          )}
        >
          <RefreshCw className={cn('h-3.5 w-3.5', rotatePending && 'animate-spin')} />
          {rotateLabel}
        </button>
        <p className="text-ink-subtle text-xs">{repairHint}</p>
      </div>
      {children}
    </section>
  )
}
