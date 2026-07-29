import { defineConfig } from 'vitest/config';
import { resolve } from 'node:path';

export default defineConfig({
  test: {
    include: ['test/**/*.test.ts'],
    // test/integration runs in a real Extension Host under mocha via
    // @vscode/test-cli, so its suite/test globals mean nothing to vitest.
    exclude: ['test/integration/**'],
    environment: 'node',
  },
  resolve: {
    alias: {
      vscode: resolve(__dirname, 'test/__mocks__/vscode.ts'),
    },
  },
});
