// The measurement benches of the model plane's validation plan (bench/**),
// run on demand and never by `npm test`, whose files match `*.test.ts`.
// M-E5 spawns a node process and headless Chrome when they are present, and
// times ten thousand objects through the store, hence the timeouts.
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['bench/**/*.bench.ts'],
    testTimeout: 1_800_000,
    hookTimeout: 120_000,
    fileParallelism: false,
    reporters: ['verbose'],
  },
});
