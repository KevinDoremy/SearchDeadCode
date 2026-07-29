const esbuild = require('esbuild');
const fs = require('fs');
const path = require('path');

const production = process.argv.includes('--production');
const watch = process.argv.includes('--watch');

const sharedOptions = {
  bundle: true,
  external: ['vscode'],
  format: 'cjs',
  platform: 'node',
  target: 'node18',
  minify: production,
  sourcemap: production ? false : 'inline',
  logLevel: 'info',
};

const extension = {
  ...sharedOptions,
  entryPoints: ['src/extension.ts'],
  outfile: 'dist/extension.js',
};

// The integration suite runs inside a real Extension Host, so mocha needs plain
// JS on disk. Guarded because the directory does not exist yet on a fresh clone.
const integrationDir = path.join('test', 'integration');
const integrationEntryPoints = fs.existsSync(integrationDir)
  ? fs
      .readdirSync(integrationDir)
      .filter(name => name.endsWith('.ts'))
      .map(name => path.join(integrationDir, name))
  : [];

const builds = [extension];
if (integrationEntryPoints.length > 0) {
  builds.push({
    ...sharedOptions,
    entryPoints: integrationEntryPoints,
    outdir: 'dist/test',
    // mocha reports the stack of a failing assertion against this file, so
    // minifying it would make every integration failure unreadable.
    minify: false,
  });
}

if (watch) {
  Promise.all(builds.map(options => esbuild.context(options).then(ctx => ctx.watch())));
} else {
  Promise.all(builds.map(options => esbuild.build(options))).catch(() => process.exit(1));
}
