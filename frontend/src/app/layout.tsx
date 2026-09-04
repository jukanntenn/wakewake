import type { Metadata } from 'next'
// Geist（Vercel 原版字体，OFL 许可随字体入库于 src/fonts/）。next/font/local
// 从仓库内 woff2 加载，构建期嵌入 out/ —— 不访问 fonts.googleapis.com，离线可构建。
import localFont from 'next/font/local'
import './globals.css'
import { Providers } from './providers'

const geist = localFont({ src: '../fonts/Geist-Variable.woff2', variable: '--font-sans' })
const geistMono = localFont({ src: '../fonts/GeistMono-Variable.woff2', variable: '--font-mono' })

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
