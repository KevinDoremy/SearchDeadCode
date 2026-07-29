import { defineConfig } from '@vscode/test-cli';

export default defineConfig({
  files: 'dist/test/**/*.test.js',
  workspaceFolder: './test/fixtures/workspace',
  mocha: {
    // The scan shells out to the CLI over a real workspace, which is slower
    // than anything vitest does here.
    timeout: 60000,
  },
});
