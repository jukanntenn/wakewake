'use client'

// 实例首页 = landing(未登录访客)。
// 内容优先:landing 直接渲染,静态导出的 index.html 即完整首屏(无 spinner 闪屏);
// 已登录用户在 effect 里 replace /devices(zustand persist 同步 rehydrate,闪一帧可接受)。
// 页根 id="landing" 供 globals.css 平滑滚动门控。

import { useEffect } from 'react'
import { useRouter } from 'next/navigation'
import { useAuthStore } from '@/stores/auth'
import { LandingHeader } from '@/components/landing/landing-header'
import { LandingHero } from '@/components/landing/landing-hero'
import { HopsSection } from '@/components/landing/hops-section'
import { TrustSection } from '@/components/landing/trust-section'
import { DeploySection } from '@/components/landing/deploy-section'
import { WhySection } from '@/components/landing/why-section'
import { LandingFooter } from '@/components/landing/landing-footer'

export default function LandingPage() {
  const user = useAuthStore((s) => s.user)
  const router = useRouter()

  useEffect(() => {
    if (user) router.replace('/devices')
  }, [user, router])

  return (
    <div id="landing" className="bg-canvas text-ink min-h-screen">
      <LandingHeader />
      <main>
        <LandingHero />
        <HopsSection />
        <TrustSection />
        <DeploySection />
        <WhySection />
      </main>
      <LandingFooter />
    </div>
  )
}
