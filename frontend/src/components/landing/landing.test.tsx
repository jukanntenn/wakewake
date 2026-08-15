import { render, screen } from '@testing-library/react'
import { NextIntlClientProvider } from 'next-intl'
import { describe, expect, it } from 'vitest'
import landingEn from '@/messages/en.json'
import { LandingHero } from './landing-hero'
import { HopsSection } from './hops-section'
import { TrustSection } from './trust-section'
import { DeploySection } from './deploy-section'
import { SpecSection } from './spec-section'
import { LandingFooter } from './landing-footer'

// 与 data-table.test.tsx 同款:局部 wrap + 真实 en.json 的 landing 命名空间(防文案漂移)。
function wrap(ui: React.ReactNode) {
  return (
    <NextIntlClientProvider locale="en" messages={{ landing: landingEn.landing }}>
      {ui}
    </NextIntlClientProvider>
  )
}

describe('landing sections', () => {
  it('hero renders headline and both CTAs with correct targets', () => {
    render(wrap(<LandingHero />))
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(
      landingEn.landing.hero.title,
    )
    const primary = screen.getByRole('link', { name: landingEn.landing.hero.ctaPrimary })
    expect(primary).toHaveAttribute('href', '/register')
    const github = screen.getByRole('link', { name: /github/i })
    expect(github).toHaveAttribute('href', expect.stringContaining('github.com'))
    expect(github).toHaveAttribute('target', '_blank')
  })

  it('exposes the three anchor targets used by the header nav', () => {
    const { container } = render(
      wrap(
        <>
          <HopsSection />
          <TrustSection />
          <DeploySection />
        </>,
      ),
    )
    expect(container.querySelector('#how-it-works')).not.toBeNull()
    expect(container.querySelector('#security')).not.toBeNull()
    expect(container.querySelector('#deploy')).not.toBeNull()
  })

  it('trust section renders all four Q&A pairs', () => {
    render(wrap(<TrustSection />))
    for (const q of [landingEn.landing.trust.q1, landingEn.landing.trust.q3]) {
      expect(screen.getByText(q)).toBeInTheDocument()
    }
    // 诚实口径:最坏情况答案必须在场
    expect(screen.getByText(new RegExp('replaying old ciphertexts'))).toBeInTheDocument()
  })

  it('spec section renders the four numbers', () => {
    render(wrap(<SpecSection />))
    expect(screen.getByText('100,000')).toBeInTheDocument()
    expect(screen.getByText('~1.2 KB')).toBeInTheDocument()
    expect(screen.getByText('2 GB')).toBeInTheDocument()
  })

  it('footer renders license and current-year copyright', () => {
    render(wrap(<LandingFooter />))
    expect(screen.getByText('AGPL-3.0')).toHaveAttribute(
      'href',
      expect.stringContaining('/LICENSE'),
    )
    expect(
      screen.getByText(new RegExp(`© ${new Date().getFullYear()} WakeWake`)),
    ).toBeInTheDocument()
  })
})
