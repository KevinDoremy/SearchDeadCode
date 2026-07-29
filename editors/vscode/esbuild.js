const esbuild = require('esbuild');

const watch = process.argv.includes('--watch');

const options = {
  entryPoints: ['src/extension.ts'],
  bundle: true,
  outfile: 'dist/extension.js',
  external: ['vscode'],
  format: 'cjs',
  platform: 'node',
  target: 'node18',
  sourcemap: !watch ? false : 'inline',
  minify: !watch,
  logLevel: 'info',
};

if (watch) {
  esbuild.context(options).then(ctx => ctx.watch());
} else {
  esbuild.build(options);
}
