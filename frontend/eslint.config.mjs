import nextCoreWebVitals from 'eslint-config-next/core-web-vitals'
import nextTypescript from 'eslint-config-next/typescript'

/** @type {import('eslint').Linter.Config[]} */
const eslintConfig = [
  {
    ignores: ['.next/**', 'dist/**', 'out/**', 'build/**', 'node_modules/**'],
  },
  ...nextCoreWebVitals,
  ...nextTypescript,
]

export default eslintConfig
