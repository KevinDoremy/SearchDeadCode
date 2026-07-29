import { spawn } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DeadCodeFinding, parseSarif } from './SarifParser';

export interface ScanOptions {
  binaryPath: string;
  workspaceRoot: string;
  minConfidence: string;
  rules: readonly string[];
  exclude: readonly string[];
  extraArgs: readonly string[];
  /** Hard cap; the CLI is fast but a huge monorepo can take a while. */
  timeoutMs?: number;
}

export class ScanError extends Error {
  constructor(message: string, readonly detail?: string) {
    super(message);
  }
}

/** Builds the argument list. Exported for testing: no spawning involved. */
export function buildArgs(options: ScanOptions, outputFile: string): string[] {
  const args = [
    options.workspaceRoot,
    '--format', 'sarif',
    '--output', outputFile,
    '--min-confidence', options.minConfidence,
  ];
  for (const glob of options.exclude) args.push('--exclude', glob);
  args.push(...options.extraArgs);
  return args;
}

/**
 * Runs one scan and returns its findings. Rejects with ScanError on a
 * missing binary, a non-zero analysis exit, a timeout, or unreadable output.
 * `signal` aborts the child process.
 */
export async function runScan(options: ScanOptions, signal?: AbortSignal): Promise<DeadCodeFinding[]> {
  if (signal?.aborted) throw new ScanError('Scan cancelled.');
  const dir = await mkdtemp(join(tmpdir(), 'searchdeadcode-'));
  const outputFile = join(dir, 'scan.sarif');
  const timeoutMs = options.timeoutMs ?? 5 * 60_000;

  try {
    const stderr = await new Promise<string>((resolve, reject) => {
      const child = spawn(options.binaryPath, buildArgs(options, outputFile), {
        cwd: options.workspaceRoot,
      });
      let stderrText = '';
      let settled = false;

      const finish = (fn: () => void) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        signal?.removeEventListener('abort', onAbort);
        fn();
      };
      const onAbort = () => {
        child.kill('SIGTERM');
        finish(() => reject(new ScanError('Scan cancelled.')));
      };
      const timer = setTimeout(() => {
        child.kill('SIGTERM');
        finish(() => reject(new ScanError(`Scan timed out after ${Math.round(timeoutMs / 1000)}s.`)));
      }, timeoutMs);

      signal?.addEventListener('abort', onAbort, { once: true });
      child.stderr?.on('data', chunk => { stderrText += String(chunk); });
      child.on('error', err => finish(() => reject(new ScanError(`Could not run searchdeadcode: ${err.message}`))));
      child.on('close', code => {
        // exit 1 just means "findings exist" unless --fail-on-findings is set;
        // exit 2 and above are real failures
        if (code !== null && code >= 2) {
          finish(() => reject(new ScanError(`searchdeadcode exited with code ${code}.`, stderrText.trim())));
        } else {
          finish(() => resolve(stderrText));
        }
      });
    });

    let sarif: string;
    try {
      sarif = await readFile(outputFile, 'utf8');
    } catch {
      throw new ScanError('searchdeadcode produced no report.', stderr.trim());
    }
    return parseSarif(sarif, options.rules);
  } catch (e) {
    if (e instanceof ScanError) throw e;
    throw new ScanError(`Unexpected output from searchdeadcode: ${(e as Error).message}`);
  } finally {
    await rm(dir, { recursive: true, force: true }).catch(() => undefined);
  }
}
