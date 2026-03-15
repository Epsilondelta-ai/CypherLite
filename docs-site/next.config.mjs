import nextra from 'nextra'

const withNextra = nextra({
  theme: 'nextra-theme-docs',
  themeConfig: './theme.config.tsx',
})

export default withNextra({
  output: 'export',
  trailingSlash: true,
  images: {
    unoptimized: true,
  },
  i18n: {
    locales: ['en', 'zh', 'hi', 'es', 'fr', 'ar', 'bn', 'pt', 'ru', 'ko'],
    defaultLocale: 'en',
  },
})
