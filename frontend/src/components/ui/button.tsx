import * as React from 'react'
import { cva, type VariantProps } from 'class-variance-authority'

import { cn } from '@/lib/utils'

const buttonVariants = cva(
  'inline-flex items-center justify-center whitespace-nowrap rounded-sm text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 focus-visible:ring-offset-canvas disabled:pointer-events-none disabled:opacity-50',
  {
    variants: {
      variant: {
        default: 'bg-primary text-on-primary hover:bg-primary/90',
        destructive: 'bg-destructive text-on-destructive hover:bg-destructive/90',
        outline: 'border border-hairline bg-canvas hover:bg-surface-2 hover:text-ink',
        secondary: 'bg-surface-2 text-ink hover:bg-surface-3',
        ghost: 'hover:bg-surface-2 hover:text-ink',
        link: 'text-primary underline-offset-4 hover:underline',
      },
      size: {
        default: 'h-10 px-4 py-2',
        sm: 'h-9 rounded-md px-3',
        lg: 'h-11 rounded-md px-8',
        icon: 'h-10 w-10',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  },
)

type ButtonProps = React.ComponentProps<'button'> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { className, variant = 'default', size = 'default', asChild = false, children, ...props },
  ref,
) {
  if (asChild && React.isValidElement(children)) {
    return React.cloneElement(children as React.ReactElement<Record<string, unknown>>, {
      'data-slot': 'button',
      'data-variant': variant,
      'data-size': size,
      className: cn(
        buttonVariants({ variant, size, className }),
        (children as React.ReactElement<{ className?: string }>).props.className,
      ),
      ...props,
    })
  }

  return (
    <button
      className={cn(buttonVariants({ variant, size, className }))}
      data-slot="button"
      data-variant={variant}
      data-size={size}
      ref={ref}
      {...props}
    >
      {children}
    </button>
  )
})

Button.displayName = 'Button'

export { Button, buttonVariants }
export type { ButtonProps }
