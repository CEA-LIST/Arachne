// The level-4 harness: one scripted headless-Chrome run against two live node
// processes (e2e/mp26.e2e.ts). Not part of `npm test`, whose files match
// `*.test.ts`; run it with `npm run e2e`.
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['e2e/**/*.e2e.ts'],
    testTimeout: 600_000,
    hookTimeout: 120_000,
    fileParallelism: false,
    reporters: ['verbose'],
  },
});
