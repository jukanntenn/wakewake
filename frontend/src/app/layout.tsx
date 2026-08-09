import type { Metadata } from 'next'
// Geist（Vercel 原版字体，MIT，DESIGN.md）。next/font/google 自托管，构建期嵌入 out/。
import { Geist, Geist_Mono } from 'next/font/google'
import './globals.css'
import { Providers } from './providers'

const geist = Geist({ variable: '--font-sans', subsets: ['latin'] })
const geistMono = Geist_Mono({ variable: '--font-mono', subsets: ['latin'] })

export const metadata: Metadata = {
  title: 'WakeWake - Wake-on-LAN Management',
  description: 'Manage your devices and send Wake-on-LAN packets',
}

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" suppressHydrationWarning className={`${geist.variable} ${geistMono.variable}`}>
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  )
}
