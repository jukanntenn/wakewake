import * as React from 'react'

import { cn } from '@/lib/utils'

function Input({ className, type, ref, ...props }: React.ComponentProps<'input'>) {
  return (
    <input
      type={type}
      data-slot="input"
      className={cn(
        'border-hairline bg-canvas ring-offset-canvas placeholder:text-ink-muted focus-visible:ring-primary flex h-10 w-full rounded-md border px-3 py-2 text-sm file:border-0 file:bg-transparent file:text-sm file:font-medium focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50',
        className,
      )}
      ref={ref}
      {...props}
    />
  )
}

export { Input }
