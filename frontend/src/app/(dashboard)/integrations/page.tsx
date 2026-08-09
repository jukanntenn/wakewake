'use client'

// Integrations 页面：Bemfa 集成的 summary/edit 双视图 + 6 态展示。
//
// 设计（方案 ε）：已连接集成默认显示只读摘要（summary）；点击"配置"切换为表单（edit），
// 淡入淡出，不撑高、不跳转、不遮罩。未连接时直接进表单（首次配置）。
// device-sync-v3：状态展示派生自后端 status（7 值）+ enabled。

// agent_offline / error / disabled。
// state-as-truth：设备/集成的增删改由 server 推 state 驱动；前端提交配置后切回 summary
// 并经 useIntegrations 轮询（syncing 态每 3s）观察连接结果。

import { useState, useMemo } from 'react'
import { useTranslations } from 'next-intl'
import { toast } from 'sonner'
import { Trash2, Power, ChevronDown, ChevronUp, AlertCircle, Info } from 'lucide-react'
import {
  useIntegrations,
  useIntegrationSchema,
  useCreateIntegration,
  usePatchIntegration,
  useDeleteIntegration,
  useToggleIntegration,
} from '@/hooks/useIntegrations'
import { useDevices } from '@/hooks/useDevices'
import { useDefaultAgent } from '@/hooks/useAgents'
import { encryptWithPublicKey } from '@/lib/crypto'
import { Button } from '@/components/ui/button'
import type { Integration } from '@/lib/api'

const BEMFA = 'bemfa'

// 非敏感字段（明文提交）；敏感字段（secret:true）RSA 加密提交。
const SECRET_FIELDS = new Set(['uid', 'secret_id', 'secret_key'])

interface JsonSchemaProperty {
  type: string
  title: string
  description?: string
  default?: string
  secret?: boolean
  pattern?: string
}

/// 展示状态（device-sync-v3 §8.5 后端直接返回 7 值 status）。
type DisplayState =
  | 'not_connected'
  | 'connecting'
  | 'connected'
  | 'error'
  | 'disconnected'
  | 'agent_offline'
  | 'disabled'

/// device-sync-v3：后端 status 已是 7 值派生，前端直接读取。
/// 仅集成不存在时（undefined）→ not_connected。
function deriveDisplayState(integration: Integration | undefined): DisplayState {
  if (!integration) return 'not_connected'
  return integration.status
}

export default function IntegrationsPage() {
  const t = useTranslations('integrations')
  const { data: integrations } = useIntegrations()
  const { data: schema } = useIntegrationSchema(BEMFA)
  const { data: agent } = useDefaultAgent()
  const { data: devices } = useDevices()
  const createMut = useCreateIntegration()
  const patchMut = usePatchIntegration()
  const deleteMut = useDeleteIntegration()
  const toggleMut = useToggleIntegration()

  const existing = integrations?.find((i) => i.provider === BEMFA)
  const displayState = deriveDisplayState(existing)
  // 设备数（用于摘要展示"已同步 N 台设备"）
  const deviceCount = devices?.length ?? 0

  const properties = useMemo(
    () => (schema?.properties ?? {}) as Record<string, JsonSchemaProperty>,
    [schema],
  )
  const required = (schema?.required ?? []) as string[]

  // v1 字段（uid）始终显示；v2 字段（secret_id/secret_key）折叠区
  const v1Keys = useMemo(
    () => Object.keys(properties).filter((k) => !SECRET_FIELDS.has(k) || k === 'uid'),
    [properties],
  )
  const v2Keys = useMemo(
    () => Object.keys(properties).filter((k) => k === 'secret_id' || k === 'secret_key'),
    [properties],
  )

  const [showV2, setShowV2] = useState(false)
  const [editing, setEditing] = useState(false)
  const [formValues, setFormValues] = useState<Record<string, string>>(() => {
    const vals: Record<string, string> = {}
    const props = (schema?.properties ?? {}) as Record<string, JsonSchemaProperty>
    for (const key of Object.keys(props)) {
      const v = existing ? (existing.config as Record<string, unknown>)[key] : undefined
      // secret 字段：existing 已脱敏为 ***，编辑时置空让用户重新输入
      vals[key] = props[key]?.secret ? '' : ((v as string) ?? props[key]?.default ?? '')
    }
    return vals
  })

  // 进入编辑模式时重置表单为当前值
  const enterEdit = () => {
    const vals: Record<string, string> = {}
    const props = (schema?.properties ?? {}) as Record<string, JsonSchemaProperty>
    for (const key of Object.keys(props)) {
      const v = existing ? (existing.config as Record<string, unknown>)[key] : undefined
      vals[key] = props[key]?.secret ? '' : ((v as string) ?? props[key]?.default ?? '')
    }
    setFormValues(vals)
    setEditing(true)
  }

  const hasV2Input = v2Keys.some((k) => formValues[k])

  // 构造提交 config：secret 字段 RSA 加密，非敏感字段明文
  const buildConfig = async (): Promise<Record<string, unknown>> => {
    if (!agent?.public_key) throw new Error('Agent not connected')
    const config: Record<string, unknown> = {}
    for (const key of Object.keys(properties)) {
      const val = formValues[key] ?? ''
      if (SECRET_FIELDS.has(key) && val) {
        config[key] = await encryptWithPublicKey(val, agent.public_key)
      } else if (!SECRET_FIELDS.has(key)) {
        config[key] = val
      }
    }
    return config
  }

  const onSave = async () => {
    try {
      const config = await buildConfig()
      if (existing) {
        // patch：保留未改的 secret（空字段不提交），非 secret 字段始终提交
        const patchConfig: Record<string, unknown> = { ...config }
        for (const key of SECRET_FIELDS) {
          if (!formValues[key]) delete patchConfig[key]
        }
        await patchMut.mutateAsync({ provider: BEMFA, data: { config: patchConfig } })
        toast.success(t('connected'))
      } else {
        await createMut.mutateAsync({ provider: BEMFA, config, enabled: true })
        toast.success(t('connected'))
      }
      setEditing(false)
    } catch {
      toast.error(t('notConnected'))
    }
  }

  const onToggle = async () => {
    if (!existing) return
    try {
      await toggleMut.mutateAsync({ provider: BEMFA, enable: !existing.enabled })
      toast.success('OK')
    } catch {
      toast.error('Failed')
    }
  }

  const onDelete = async () => {
    if (!existing) return
    if (!confirm(t('deleteConfirm'))) return
    try {
      await deleteMut.mutateAsync(BEMFA)
      toast.success(t('deleteRemoved'))
      setEditing(false)
    } catch {
      toast.error('Failed')
    }
  }

  // 未连接 OR 编辑中 → 表单视图
  const showForm = displayState === 'not_connected' || editing

  return (
    <div className="space-y-6">
      <h1 className="text-ink text-2xl font-semibold tracking-tight">{t('title')}</h1>

      <div className="border-hairline bg-surface-1 shadow-card rounded-lg border p-6">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h2 className="text-ink font-medium">Bemfa</h2>
            {existing && (
              <div className="mt-1.5">
                <StatusBadge state={displayState} />
              </div>
            )}
            {!existing && <p className="text-ink-muted mt-1 text-sm">{t('bemfaDesc')}</p>}
          </div>
          {existing && !showForm && (
            <div className="flex shrink-0 gap-2">
              <button
                onClick={onToggle}
                className="border-hairline text-ink-muted hover:bg-surface-2 flex items-center gap-1 rounded-sm border px-3 py-1.5 text-sm"
              >
                <Power className="h-3.5 w-3.5" />
                {existing.enabled ? t('disable') : t('enable')}
              </button>
              <button
                onClick={enterEdit}
                className="border-hairline text-ink-muted hover:bg-surface-2 flex items-center gap-1 rounded-sm border px-3 py-1.5 text-sm"
              >
                {t('edit')}
              </button>
              <button
                onClick={onDelete}
                className="text-ink-muted hover:bg-surface-2 hover:text-destructive rounded-sm p-1.5"
              >
                <Trash2 className="h-4 w-4" />
              </button>
            </div>
          )}
        </div>

        {/* 错误信息展示（仅 error/disconnected 态） */}
        {existing?.last_error && (displayState === 'error' || displayState === 'disconnected') && (
          <div className="border-hairline bg-destructive/5 mt-3 flex items-start gap-2 rounded-sm border border-dashed p-3">
            <AlertCircle className="text-destructive mt-0.5 h-4 w-4 shrink-0" />
            <span className="text-destructive text-sm">
              {t('lastError', { error: existing.last_error })}
            </span>
          </div>
        )}

        {/* summary 视图：已连接的只读摘要 */}
        {existing && !showForm && (
          <SummaryView
            state={displayState}
            deviceCount={deviceCount}
            stateHint={t(
              displayState === 'connecting'
                ? 'stateConnectingHint'
                : displayState === 'agent_offline'
                  ? 'stateAgentOfflineHint'
                  : displayState === 'disconnected'
                    ? 'stateDisconnectedHint'
                    : displayState === 'disabled'
                      ? 'stateDisabledHint'
                      : 'stateConnectingHint',
            )}
          />
        )}

        {/* 表单视图：首次配置 OR 编辑 */}
        {showForm && (
          <div className="mt-4 space-y-3">
            {existing && <p className="text-ink-muted text-sm">{t('editConfig')}</p>}
            {/* v1 字段：uid */}
            <div className="space-y-3">
              {v1Keys.map((key) => (
                <FormField
                  key={key}
                  propKey={key}
                  prop={properties[key]}
                  value={formValues[key] ?? ''}
                  required={required.includes(key)}
                  isExisting={!!existing}
                  onChange={(v) => setFormValues({ ...formValues, [key]: v })}
                />
              ))}
            </div>

            {/* v2 折叠区 */}
            {v2Keys.length > 0 && (
              <div className="border-hairline rounded-sm border">
                <button
                  onClick={() => setShowV2(!showV2)}
                  className="text-ink-muted hover:bg-surface-2 flex w-full items-center justify-between px-3 py-2 text-sm"
                >
                  <span className="flex items-center gap-1">
                    {showV2 ? (
                      <ChevronUp className="h-4 w-4" />
                    ) : (
                      <ChevronDown className="h-4 w-4" />
                    )}
                    {t('v2Toggle')}
                  </span>
                  {hasV2Input && <span className="text-primary text-xs">v2</span>}
                </button>
                {showV2 && (
                  <div className="border-hairline space-y-3 border-t p-3">
                    <p className="text-ink-subtle text-xs">{t('v2Hint')}</p>
                    {v2Keys.map((key) => (
                      <FormField
                        key={key}
                        propKey={key}
                        prop={properties[key]}
                        value={formValues[key] ?? ''}
                        required={false}
                        isExisting={!!existing}
                        onChange={(v) => setFormValues({ ...formValues, [key]: v })}
                      />
                    ))}
                  </div>
                )}
              </div>
            )}

            <p className="text-ink-subtle text-xs">{t('configHint')}</p>

            <div className="flex justify-end gap-2">
              {existing && (
                <Button
                  variant="ghost"
                  onClick={() => setEditing(false)}
                  disabled={patchMut.isPending}
                >
                  {t('cancel')}
                </Button>
              )}
              <Button onClick={onSave} disabled={createMut.isPending || patchMut.isPending}>
                {existing ? t('save') : t('connect')}
              </Button>
            </div>
          </div>
        )}
      </div>

      {/* 米家绑定指引：仅 connected 态显示 */}
      {(displayState === 'connected' || displayState === 'error') && (
        <div className="border-hairline bg-surface-1 shadow-card rounded-lg border p-6">
          <h3 className="text-ink flex items-center gap-2 font-medium">
            <Info className="text-primary h-4 w-4" />
            {t('mijiaGuide')}
          </h3>
          <ol className="text-ink-muted mt-3 space-y-2 text-sm">
            <li>1. {t('mijiaStep1')}</li>
            <li>2. {t('mijiaStep2')}</li>
            <li>3. {t('mijiaStep3')}</li>
            <li>4. {t('mijiaStep4', { device: 'XXX' })}</li>
          </ol>
          <div className="border-hairline bg-warning/5 mt-3 flex items-start gap-2 rounded-sm border border-dashed p-3">
            <AlertCircle className="text-warning mt-0.5 h-4 w-4 shrink-0" />
            <span className="text-warning text-sm">{t('mijiaWarning')}</span>
          </div>
        </div>
      )}
    </div>
  )
}

/// 状态徽章：根据展示态显示颜色 + 文案。
function StatusBadge({ state }: { state: DisplayState }) {
  const t = useTranslations('integrations')
  const config: Record<DisplayState, { color: string; label: string; icon?: boolean }> = {
    connected: { color: 'bg-success', label: t('stateConnected') },
    connecting: { color: 'bg-primary', label: t('stateConnecting'), icon: true },
    error: { color: 'bg-destructive', label: t('stateError') },
    disconnected: { color: 'bg-warning', label: t('stateDisconnected') },
    agent_offline: { color: 'bg-ink-subtle', label: t('stateAgentOffline') },
    disabled: { color: 'bg-ink-subtle', label: t('stateDisabled') },
    not_connected: { color: 'bg-ink-subtle', label: t('notConnected') },
  }
  const c = config[state]
  return (
    <span className="inline-flex items-center gap-1.5 text-sm">
      <span className={`inline-block h-2 w-2 rounded-full ${c.color}`} />
      <span className="text-ink-muted">{c.label}</span>
    </span>
  )
}

/// summary 视图：已连接的只读摘要（设备数 + 状态提示）。
function SummaryView({
  state,
  deviceCount,
  stateHint,
}: {
  state: DisplayState
  deviceCount: number
  stateHint: string
}) {
  const t = useTranslations('integrations')
  return (
    <div className="mt-4 space-y-2">
      {(state === 'connected' || state === 'connecting') && deviceCount > 0 && (
        <p className="text-ink-muted text-sm">{t('devicesSynced', { count: deviceCount })}</p>
      )}
      {state !== 'connected' && <p className="text-ink-subtle text-xs">{stateHint}</p>}
    </div>
  )
}

/// 单个表单字段（schema 驱动）。
function FormField({
  propKey,
  prop,
  value,
  required,
  isExisting,
  onChange,
}: {
  propKey: string
  prop: JsonSchemaProperty
  value: string
  required: boolean
  isExisting: boolean
  onChange: (v: string) => void
}) {
  const t = useTranslations('integrations')
  const isSecret = SECRET_FIELDS.has(propKey)
  return (
    <div className="space-y-1">
      <label className="text-ink-muted text-sm">
        {prop.title}
        {required && <span className="text-destructive"> *</span>}
        {isSecret && <span className="text-warning ml-1 text-xs">(encrypted)</span>}
      </label>
      <input
        type={isSecret ? 'password' : 'text'}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={isSecret && isExisting ? t('placeholderSecret') : (prop.description ?? '')}
        className="border-hairline bg-surface-1 text-ink focus:border-primary w-full rounded-sm border px-3 py-2 outline-none"
      />
    </div>
  )
}
