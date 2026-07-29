import { defineConfig } from 'vitest/config';
import { resolve } from 'node:path';

export default defineConfig({
  test: {
    include: ['test/**/*.test.ts'],
    environment: 'node',
  },
  resolve: {
    alias: {
      vscode: resolve(__dirname, 'test/__mocks__/vscode.ts'),
    },
  },
});
