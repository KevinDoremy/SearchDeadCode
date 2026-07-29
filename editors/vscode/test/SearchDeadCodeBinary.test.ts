import { describe, it, expect } from 'vitest';
import { join } from 'node:path';
import {
  resolveBinary,
  parseVersion,
  isAtLeast,
  bundledBinaryPath,
  MIN_VERSION,
} from '../src/SearchDeadCodeBinary';

describe('version helpers', () => {
  it('parses the CLI banner', () => {
    expect(parseVersion('searchdeadcode 0.13.0\n')).toEqual([0, 13, 0]);
    expect(parseVersion('0.10')).toEqual([0, 10, 0]);
    expect(parseVersion('nonsense')).toEqual([0, 0, 0]);
  });

  it('compares versions componentwise, not lexically', () => {
    expect(isAtLeast('0.10.0', '0.9.0')).toBe(true); // 10 > 9 despite the string order
    expect(isAtLeast('0.13.0', '0.10.0')).toBe(true);
    expect(isAtLeast('0.10.0', '0.10.0')).toBe(true);
    expect(isAtLeast('0.9.9', '0.10.0')).toBe(false);
  });
});

describe('resolveBinary', () => {
  const probeOk = (version: string) => async () => `searchdeadcode ${version}\n`;

  it('prefers the configured path', async () => {
    const seen: string[] = [];
    const info = await resolveBinary({
      configuredPath: '/custom/sdc',
      fileExists: () => true,
      probeVersion: async c => { seen.push(c); return 'searchdeadcode 0.13.0'; },
    });
    expect(info?.path).toBe('/custom/sdc');
    expect(seen).toEqual(['/custom/sdc']);
  });

  it('falls back to PATH when nothing is configured', async () => {
    const info = await resolveBinary({ configuredPath: '', fileExists: () => false, probeVersion: probeOk('0.13.0') });
    expect(info?.path).toBe('searchdeadcode');
  });

  it('tries Homebrew and cargo when PATH does not have it', async () => {
    const info = await resolveBinary({
      configuredPath: '',
      fileExists: p => p === '/opt/homebrew/bin/searchdeadcode',
      probeVersion: async c => {
        if (c === 'searchdeadcode') throw new Error('ENOENT');
        return 'searchdeadcode 0.13.0';
      },
    });
    expect(info?.path).toBe('/opt/homebrew/bin/searchdeadcode');
  });

  it('reports an older binary as found but unsupported', async () => {
    const info = await resolveBinary({ configuredPath: '', fileExists: () => false, probeVersion: probeOk('0.9.0') });
    expect(info).toMatchObject({ version: '0.9.0', supported: false });
  });

  it('accepts the currently shipped Homebrew version', async () => {
    const info = await resolveBinary({ configuredPath: '', fileExists: () => false, probeVersion: probeOk('0.10.0') });
    expect(info?.supported).toBe(true);
    expect(MIN_VERSION).toBe('0.10.0');
  });

  it('returns undefined when no candidate answers', async () => {
    const info = await resolveBinary({
      configuredPath: '/nope',
      fileExists: () => false,
      probeVersion: async () => { throw new Error('ENOENT'); },
    });
    expect(info).toBeUndefined();
  });

  it('ignores a configured path made of whitespace', async () => {
    const info = await resolveBinary({ configuredPath: '   ', fileExists: () => false, probeVersion: probeOk('0.13.0') });
    expect(info?.path).toBe('searchdeadcode');
  });
});

describe('bundled binary', () => {
  const bundled = bundledBinaryPath('/ext');

  it('lives under bin/ in the extension directory', () => {
    expect(bundled.startsWith(join('/ext', 'bin'))).toBe(true);
  });

  it('wins over PATH, so a platform VSIX works with no CLI installed', async () => {
    const info = await resolveBinary({
      configuredPath: '',
      extensionPath: '/ext',
      fileExists: p => p === bundled,
      probeVersion: async () => 'searchdeadcode 0.13.0',
    });
    expect(info?.path).toBe(bundled);
  });

  it('loses to a configured path, so a local build still wins', async () => {
    const info = await resolveBinary({
      configuredPath: '/custom/sdc',
      extensionPath: '/ext',
      fileExists: () => true,
      probeVersion: async () => 'searchdeadcode 0.13.0',
    });
    expect(info?.path).toBe('/custom/sdc');
  });

  it('falls through to PATH on the generic VSIX, which ships no binary', async () => {
    const info = await resolveBinary({
      configuredPath: '',
      extensionPath: '/ext',
      fileExists: () => false,
      probeVersion: async () => 'searchdeadcode 0.13.0',
    });
    expect(info?.path).toBe('searchdeadcode');
  });

  it('skips a bundled binary that exists but cannot run', async () => {
    const info = await resolveBinary({
      configuredPath: '',
      extensionPath: '/ext',
      fileExists: p => p === bundled,
      probeVersion: async c => {
        if (c === bundled) throw new Error('EACCES'); // lost its +x bit
        return 'searchdeadcode 0.13.0';
      },
    });
    expect(info?.path).toBe('searchdeadcode');
  });
});
