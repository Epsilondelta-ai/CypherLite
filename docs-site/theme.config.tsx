import type { DocsThemeConfig } from 'nextra-theme-docs'

const config: DocsThemeConfig = {
  logo: (
    <span style={{ display: 'flex', alignItems: 'center', gap: '0.5em' }}>
      <img src="/CypherLite/logo.png" alt="CypherLite" width={32} height={32} style={{ borderRadius: '4px' }} />
      <span style={{ fontWeight: 800, fontSize: '1.2em' }}>CypherLite</span>
    </span>
  ),
  project: {
    link: 'https://github.com/Epsilondelta-ai/CypherLite',
  },
  docsRepositoryBase:
    'https://github.com/Epsilondelta-ai/CypherLite/tree/main/docs-site',
  i18n: [
    { locale: 'en', name: 'English' },
    { locale: 'zh', name: '中文' },
    { locale: 'hi', name: 'हिन्दी' },
    { locale: 'es', name: 'Español' },
    { locale: 'fr', name: 'Français' },
    { locale: 'ar', name: 'العربية', direction: 'rtl' },
    { locale: 'bn', name: 'বাংলা' },
    { locale: 'pt', name: 'Português' },
    { locale: 'ru', name: 'Русский' },
    { locale: 'ko', name: '한국어' },
  ],
  head: (
    <>
      <meta name="viewport" content="width=device-width, initial-scale=1.0" />
      <meta
        name="description"
        content="CypherLite - SQLite-like simplicity for graph databases"
      />
      <meta
        property="og:title"
        content="CypherLite Documentation"
      />
      <meta
        property="og:description"
        content="A lightweight, embedded, single-file graph database engine written in Rust"
      />
      <meta property="og:image" content="/CypherLite/og-image.png" />
      <meta property="og:type" content="website" />
      <link rel="icon" href="/favicon.ico" />
    </>
  ),
  sidebar: {
    defaultMenuCollapseLevel: 1,
    toggleButton: true,
  },
  toc: {
    backToTop: true,
  },
  footer: {
    content: (
      <span>
        MIT / Apache-2.0 {new Date().getFullYear()} &copy;{' '}
        <a href="https://github.com/Epsilondelta-ai" target="_blank" rel="noreferrer">
          CypherLite Contributors
        </a>
      </span>
    ),
  },
  editLink: {
    content: 'Edit this page on GitHub',
  },
  feedback: {
    content: 'Question? Give us feedback',
    labels: 'feedback',
  },
  navigation: {
    prev: true,
    next: true,
  },
  darkMode: true,
  color: {
    hue: 210,
    saturation: 80,
  },
}

export default config
