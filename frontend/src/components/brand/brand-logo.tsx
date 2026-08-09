// WakeWake 品牌标识：方块容器内的闪电符号（呼应 WoL 唤醒语义）。
// 用于 auth 页面与 header。纯 inline SVG，无依赖。

interface BrandLogoProps {
  size?: number
  className?: string
}

export function BrandMark({ size = 24, className = '' }: BrandLogoProps) {
  return (
    <span
      className={`inline-flex items-center justify-center rounded-md text-white ${className}`}
      style={{ width: size, height: size, backgroundColor: 'oklch(14.9% 0.024 285.8)' }}
      aria-hidden="true"
    >
      <svg
        width={size * 0.6}
        height={size * 0.6}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={2.5}
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        {/* 闪电：唤醒的视觉隐喻 */}
        <path d="M13 2 3 14h9l-1 8 10-12h-9l1-8z" />
      </svg>
    </span>
  )
}

export function BrandLogo({
  size = 24,
  withWordmark = true,
}: {
  size?: number
  withWordmark?: boolean
}) {
  return (
    <span className="flex items-center gap-2">
      <BrandMark size={size} />
      {withWordmark && (
        <span className="text-ink text-lg font-semibold tracking-tight">WakeWake</span>
      )}
    </span>
  )
}
