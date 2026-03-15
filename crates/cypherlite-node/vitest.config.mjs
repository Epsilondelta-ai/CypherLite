import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['__test__/**/*.spec.{js,mjs,ts}'],
    testTimeout: 30000,
  },
});
