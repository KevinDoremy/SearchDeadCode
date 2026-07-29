import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { chmodSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { buildArgs, runScan, ScanError } from '../src/SearchDeadCodeService';

/** Stand-in binary that ignores every argument and sleeps, like a slow scan. */
let sleeperDir: string;
let sleeper: string;
beforeAll(() => {
  sleeperDir = mkdtempSync(join(tmpdir(), 'sdc-test-'));
  sleeper = join(sleeperDir, 'slow-scan');
  writeFileSync(sleeper, '#!/bin/sh\nsleep 30\n');
  chmodSync(sleeper, 0o755);
});
afterAll(() => rmSync(sleeperDir, { recursive: true, force: true }));

const base = {
  binaryPath: 'searchdeadcode',
  workspaceRoot: '/ws',
  minConfidence: 'medium',
  rules: ['DC001'],
  exclude: [],
  extraArgs: [],
};

describe('buildArgs', () => {
  it('puts the root first and always asks for SARIF into the given file', () => {
    expect(buildArgs(base, '/tmp/out.sarif')).toEqual([
      '/ws', '--format', 'sarif', '--output', '/tmp/out.sarif', '--min-confidence', 'medium',
    ]);
  });

  it('repeats --exclude per glob and appends extraArgs last', () => {
    const args = buildArgs(
      { ...base, exclude: ['**/build/**', '**/generated/**'], extraArgs: ['--deep', 'false'] },
      '/tmp/o.sarif',
    );
    expect(args.filter(a => a === '--exclude')).toHaveLength(2);
    expect(args.slice(-2)).toEqual(['--deep', 'false']);
    // the rule allowlist is applied when parsing, never passed to the CLI
    expect(args).not.toContain('DC001');
  });

  it('passes the confidence level through', () => {
    expect(buildArgs({ ...base, minConfidence: 'high' }, '/tmp/o.sarif')).toContain('high');
  });
});

describe('runScan failure modes', () => {
  it('reports a missing binary as a ScanError instead of crashing', async () => {
    await expect(
      runScan({ ...base, binaryPath: '/definitely/not/here/sdc', workspaceRoot: process.cwd() }),
    ).rejects.toBeInstanceOf(ScanError);
  });

  it('refuses to spawn anything once the scan is cancelled', async () => {
    const controller = new AbortController();
    controller.abort();
    await expect(
      runScan({ ...base, binaryPath: 'sleep', workspaceRoot: process.cwd() }, controller.signal),
    ).rejects.toThrow(/cancelled/);
  });

  it('kills a running scan when cancelled mid-flight', async () => {
    const controller = new AbortController();
    const promise = runScan(
      { ...base, binaryPath: sleeper, workspaceRoot: process.cwd(), timeoutMs: 30_000 },
      controller.signal,
    );
    setTimeout(() => controller.abort(), 50);
    await expect(promise).rejects.toThrow(/cancelled/);
  });

  it('gives up on a scan that outlives its timeout', async () => {
    await expect(
      runScan({ ...base, binaryPath: sleeper, workspaceRoot: process.cwd(), timeoutMs: 150 }),
    ).rejects.toThrow(/timed out/);
  });

  it('treats a fast exit with no report as a ScanError, not a silent empty result', async () => {
    await expect(
      runScan({ ...base, binaryPath: 'true', workspaceRoot: process.cwd() }),
    ).rejects.toThrow(/no report/);
  });
});
