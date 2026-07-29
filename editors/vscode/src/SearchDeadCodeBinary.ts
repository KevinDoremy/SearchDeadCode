import { execFile } from 'node:child_process';
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';

const run = promisify(execFile);

/** Minimum CLI version whose SARIF output this extension understands. */
export const MIN_VERSION = '0.10.0';

/**
 * VS Code on macOS inherits a login shell PATH that often lacks Homebrew and
 * cargo, so a bare `searchdeadcode` lookup fails even when the binary is
 * installed. These are checked after PATH.
 */
const FALLBACK_DIRS = [
  '/opt/homebrew/bin',
  '/usr/local/bin',
  join(homedir(), '.cargo', 'bin'),
  join(homedir(), '.local', 'bin'),
];

export interface BinaryInfo {
  path: string;
  version: string;
  /** False when the binary is older than MIN_VERSION. */
  supported: boolean;
}

/** `1.2.3` → [1, 2, 3]; missing parts read as 0. */
export function parseVersion(text: string): [number, number, number] {
  const m = /(\d+)\.(\d+)(?:\.(\d+))?/.exec(text);
  if (!m) return [0, 0, 0];
  return [Number(m[1]), Number(m[2]), Number(m[3] ?? 0)];
}

export function isAtLeast(version: string, minimum: string): boolean {
  const a = parseVersion(version);
  const b = parseVersion(minimum);
  for (let i = 0; i < 3; i++) {
    if (a[i] > b[i]) return true;
    if (a[i] < b[i]) return false;
  }
  return true;
}

export interface ResolveDeps {
  /** Configured absolute path, empty when unset. */
  configuredPath: string;
  fileExists?: (p: string) => boolean;
  /** Runs `<candidate> --version`, resolving its stdout. Rejects if unusable. */
  probeVersion?: (candidate: string) => Promise<string>;
}

async function defaultProbe(candidate: string): Promise<string> {
  const { stdout } = await run(candidate, ['--version'], { timeout: 10_000 });
  return stdout;
}

/**
 * Finds a usable binary: configured path, then PATH, then the fallback dirs.
 * Returns undefined when nothing answers `--version`.
 */
export async function resolveBinary(deps: ResolveDeps): Promise<BinaryInfo | undefined> {
  const fileExists = deps.fileExists ?? existsSync;
  const probeVersion = deps.probeVersion ?? defaultProbe;

  const candidates: string[] = [];
  if (deps.configuredPath.trim()) candidates.push(deps.configuredPath.trim());
  candidates.push('searchdeadcode'); // PATH
  for (const dir of FALLBACK_DIRS) {
    const p = join(dir, 'searchdeadcode');
    if (fileExists(p)) candidates.push(p);
  }

  for (const candidate of candidates) {
    try {
      const stdout = await probeVersion(candidate);
      const version = parseVersion(stdout).join('.');
      return { path: candidate, version, supported: isAtLeast(version, MIN_VERSION) };
    } catch {
      // not here, try the next candidate
    }
  }
  return undefined;
}
