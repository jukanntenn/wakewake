'use client'

// DeviceForm：添加/编辑设备表单（api-design.md §1.4 + Part 3 MAC 加密）。
// 前端用 agent RSA 公钥（Web Crypto RSA-OAEP/SHA-256）加密 MAC，server 永不见明文。
// react-hook-form + garde 风格校验。

import { useState } from 'react'
import { useForm } from 'react-hook-form'
import { useTranslations } from 'next-intl'
import { useDefaultAgent } from '@/hooks/useAgents'
import { useCreateDevice, usePatchDevice } from '@/hooks/useDevices'
import { encryptWithPublicKey, maskMACAddress, isValidMAC, formatMAC } from '@/lib/crypto'
import { type Device, type CreateDeviceInput, type FieldError, ApiError } from '@/lib/api'

interface FormValues {
  name: string
  mac: string
  description: string
}

interface DeviceFormProps {
  device?: Device | null
  onDone: () => void
}

export function DeviceForm({ device, onDone }: DeviceFormProps) {
  const t = useTranslations('device')
  const tErr = useTranslations('validation')
  const tCode = useTranslations('error')
  const { data: agent } = useDefaultAgent()
  const createMut = useCreateDevice()
  const patchMut = usePatchDevice()
  const isEdit = !!device

  const {
    register,
    handleSubmit,
    formState: { errors },
  } = useForm<FormValues>({
    defaultValues: {
      name: device?.name ?? '',
      mac: '',
      description: device?.description ?? '',
    },
  })
  const [submitErr, setSubmitErr] = useState<string | null>(null)

  const onSubmit = async (values: FormValues) => {
    setSubmitErr(null)
    try {
      if (isEdit && device) {
        // 编辑：MAC 仅在用户重新输入时更新（公钥变更场景）
        const patch: Record<string, unknown> = {
          name: values.name,
          description: values.description || null,
        }
        if (values.mac && isValidMAC(values.mac)) {
          if (!agent?.public_key) throw new Error(t('connectorNotReady'))
          const macEncrypted = await encryptWithPublicKey(formatMAC(values.mac), agent.public_key)
          patch.mac_encrypted = macEncrypted
          patch.mac_display = maskMACAddress(formatMAC(values.mac))
        }
        await patchMut.mutateAsync({ did: device.did, data: patch })
      } else {
        // 创建：必须加密 MAC
        if (!agent?.public_key) throw new Error(t('connectorNotReady'))
        const formatted = formatMAC(values.mac)
        if (!isValidMAC(formatted)) throw new Error(t('invalidMac'))
        const macEncrypted = await encryptWithPublicKey(formatted, agent.public_key)
        const input: CreateDeviceInput = {
          name: values.name,
          mac_encrypted: macEncrypted,
          mac_display: maskMACAddress(formatted),
          description: values.description || undefined,
        }
        await createMut.mutateAsync(input)
      }
      onDone()
    } catch (err) {
      if (err instanceof ApiError) {
        setSubmitErr(mapApiErrorToMessage(err, tErr, tCode, t))
      } else {
        setSubmitErr(err instanceof Error ? err.message : String(err))
      }
    }
  }

  const loading = createMut.isPending || patchMut.isPending

  return (
    <form onSubmit={handleSubmit(onSubmit)} className="space-y-4">
      <div className="space-y-1">
        <label className="text-ink-muted text-sm">{t('name')}</label>
        <input
          {...register('name', { required: true, maxLength: 100 })}
          className="border-hairline bg-surface-1 text-ink focus:border-primary w-full rounded-sm border px-3 py-2 outline-none"
          placeholder={t('namePlaceholder')}
        />
        {errors.name && (
          <span className="text-destructive text-xs">{tErr('required', { field: t('name') })}</span>
        )}
      </div>
      <div className="space-y-1">
        <label className="text-ink-muted text-sm">{t('macAddress')}</label>
        <input
          {...register('mac')}
          className="border-hairline bg-surface-1 text-ink focus:border-primary w-full rounded-sm border px-3 py-2 font-mono outline-none"
          placeholder={t('macAddressPlaceholder')}
          disabled={isEdit}
        />
        {isEdit && <span className="text-ink-subtle text-xs">{t('macLockedOnEdit')}</span>}
        {!isEdit && <span className="text-ink-subtle text-xs">{t('encryptionHint')}</span>}
      </div>
      <div className="space-y-1">
        <label className="text-ink-muted text-sm">{t('description')}</label>
        <input
          {...register('description', { maxLength: 255 })}
          className="border-hairline bg-surface-1 text-ink focus:border-primary w-full rounded-sm border px-3 py-2 outline-none"
          placeholder={t('descriptionPlaceholder')}
        />
      </div>
      {submitErr && <p className="text-destructive text-sm">{submitErr}</p>}
      <div className="flex justify-end gap-2">
        <button
          type="button"
          onClick={onDone}
          className="text-ink-muted hover:bg-surface-2 rounded-sm px-4 py-2 text-sm"
        >
          {t('cancel')}
        </button>
        <button
          type="submit"
          disabled={loading}
          className="bg-primary text-on-primary rounded-sm px-4 py-2 text-sm font-medium disabled:opacity-50"
        >
          {loading ? t('saving') : isEdit ? t('save') : t('create')}
        </button>
      </div>
    </form>
  )
}

/// 把 ApiError 映射成可读文案：优先字段级 → 业务码 → 通用。
function mapApiErrorToMessage(
  err: ApiError,
  tErr: (key: string, params?: Record<string, string>) => string,
  tCode: (key: string) => string,
  t: (key: string) => string,
): string {
  // 字段级校验错误（VALIDATION_FAILED + errors[]）：取第一条。
  if (err.errors && err.errors.length > 0) {
    const fe: FieldError = err.errors[0]
    if (fe.code === 'required') return tErr('required', { field: fieldLabel(fe.field, t) })
    if (fe.code === 'min_length' || fe.code === 'max_length' || fe.code === 'length')
      return tErr(fe.code, { field: fieldLabel(fe.field, t) })
    return tErr('invalid_format', { field: fieldLabel(fe.field, t) })
  }
  // 业务错误码 → error namespace。
  const codeMap: string[] = [
    'QUOTA_EXCEEDED',
    'DEVICE_NOT_FOUND',
    'AGENT_NOT_FOUND',
    'SYNCING',
    'RATE_LIMITED',
  ]
  if (codeMap.includes(err.code)) return tCode(err.code)
  return err.message || tErr('invalid_format', { field: t('name') })
}

/// 字段路径 → i18n 字段名（name / macAddress / description）。
function fieldLabel(field: string, t: (key: string) => string): string {
  if (field.includes('mac')) return t('macAddress')
  if (field.includes('description')) return t('description')
  return t('name')
}
