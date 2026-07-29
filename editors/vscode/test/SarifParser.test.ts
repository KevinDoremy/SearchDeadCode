import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { parseSarif, resolveFindingPath, SarifParseError } from '../src/SarifParser';

const sample = () => readFileSync(join(__dirname, 'fixtures', 'sample.sarif'), 'utf8');

describe('parseSarif', () => {
  it('reads the three well-formed results and drops the malformed one', () => {
    const found = parseSarif(sample());
    expect(found.map(f => f.ruleId)).toEqual(['DC001', 'DC004', 'AP023']);
  });

  it('converts SARIF 1-based positions to 0-based', () => {
    const [dc001] = parseSarif(sample());
    expect(dc001.line).toBe(14);
    expect(dc001.character).toBe(0);
  });

  it('defaults a missing startColumn to the start of the line', () => {
    const dc004 = parseSarif(sample()).find(f => f.ruleId === 'DC004')!;
    expect(dc004.character).toBe(0);
  });

  it('falls back to the rule default level when the result omits it', () => {
    const dc004 = parseSarif(sample()).find(f => f.ruleId === 'DC004')!;
    expect(dc004.level).toBe('note');
  });

  it('carries helpUri and fingerprint when present, undefined otherwise', () => {
    const [dc001, dc004] = parseSarif(sample());
    expect(dc001.helpUri).toContain('#dc001');
    expect(dc001.fingerprint).toBe('a1b2c3');
    expect(dc004.helpUri).toBeUndefined();
    expect(dc004.fingerprint).toBeUndefined();
  });

  it('reads a whole-line deletion fix as a 0-based inclusive range', () => {
    const [dc001] = parseSarif(sample());
    expect(dc001.fix).toEqual({ startLine: 14, endLine: 41 });
  });

  it('ignores a "fix" that inserts text instead of deleting', () => {
    const doc = JSON.parse(sample());
    doc.runs[0].results[0].fixes[0].artifactChanges[0].replacements[0].insertedContent.text = 'replacement';
    expect(parseSarif(JSON.stringify(doc))[0].fix).toBeUndefined();
  });

  it('applies the rule allowlist', () => {
    const found = parseSarif(sample(), ['DC001', 'DC004']);
    expect(found.map(f => f.ruleId)).toEqual(['DC001', 'DC004']);
    expect(parseSarif(sample(), []).length).toBe(3); // empty allowlist keeps everything
  });

  it('survives 0.10-era output with no rules[], fingerprints or fixes', () => {
    const legacy = JSON.stringify({
      version: '2.1.0',
      runs: [
        {
          tool: { driver: { name: 'searchdeadcode', version: '0.10.0' } },
          results: [
            {
              ruleId: 'DC001',
              level: 'warning',
              message: { text: "class 'OrphanHelper' is never used" },
              locations: [
                { physicalLocation: { artifactLocation: { uri: 'p/src/Orphan.kt' }, region: { startLine: 3, startColumn: 1 } } },
              ],
            },
          ],
        },
      ],
    });
    const [f] = parseSarif(legacy);
    expect(f).toMatchObject({ ruleId: 'DC001', line: 2, character: 0, level: 'warning' });
    expect(f.fix).toBeUndefined();
    expect(f.helpUri).toBeUndefined();
  });

  it('rejects non-JSON, a non-2.1 version and a missing runs[]', () => {
    expect(() => parseSarif('not json')).toThrow(SarifParseError);
    expect(() => parseSarif('{"version":"3.0.0","runs":[]}')).toThrow(/unsupported SARIF version/);
    expect(() => parseSarif('{"version":"2.1.0"}')).toThrow(/missing runs/);
    expect(parseSarif('{"version":"2.1.0","runs":[]}')).toEqual([]);
  });

  it('never throws on structurally odd results', () => {
    const odd = JSON.stringify({
      version: '2.1.0',
      runs: [{ results: [null, 42, { ruleId: 5 }, { ruleId: 'DC001', locations: [] }] }],
    });
    expect(parseSarif(odd)).toEqual([]);
  });
});

describe('resolveFindingPath', () => {
  const existing = (paths: string[]) => (p: string) => paths.includes(p);

  it('joins a plain relative path to the workspace root', () => {
    const p = resolveFindingPath('/ws/proj', 'src/Main.kt', existing(['/ws/proj/src/Main.kt']));
    expect(p).toBe('/ws/proj/src/Main.kt');
  });

  it('strips the root basename when the CLI reports paths from the parent', () => {
    // the real 0.10 behaviour: scanning /ws/proj reports "proj/src/Main.kt"
    const p = resolveFindingPath('/ws/proj', 'proj/src/Main.kt', existing(['/ws/proj/src/Main.kt']));
    expect(p).toBe('/ws/proj/src/Main.kt');
  });

  it('prefers the direct join when both candidates exist', () => {
    const p = resolveFindingPath('/ws/proj', 'proj/src/Main.kt', existing(['/ws/proj/proj/src/Main.kt', '/ws/proj/src/Main.kt']));
    expect(p).toBe('/ws/proj/proj/src/Main.kt');
  });

  it('returns absolute paths untouched', () => {
    expect(resolveFindingPath('/ws/proj', '/elsewhere/A.kt', () => false)).toBe('/elsewhere/A.kt');
  });

  it('falls back to the direct join rather than dropping the finding', () => {
    expect(resolveFindingPath('/ws/proj', 'src/Gone.kt', () => false)).toBe('/ws/proj/src/Gone.kt');
  });

  it('tolerates a trailing slash on the root', () => {
    expect(resolveFindingPath('/ws/proj/', 'src/Main.kt', existing(['/ws/proj/src/Main.kt']))).toBe('/ws/proj/src/Main.kt');
  });
});
