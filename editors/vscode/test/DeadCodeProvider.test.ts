import { describe, it, expect } from 'vitest';
import {
  appendToBaseline,
  buildIgnoreComment,
  describeFinding,
  toBaselineEntry,
  toDiagnostic,
  DIAGNOSTIC_SOURCE,
} from '../src/DeadCodeProvider';
import { DiagnosticSeverity, DiagnosticTag } from './__mocks__/vscode';
import type { DeadCodeFinding } from '../src/SarifParser';

const finding = (over: Partial<DeadCodeFinding> = {}): DeadCodeFinding => ({
  ruleId: 'DC001',
  message: "class 'LegacyEncoder' is never used",
  level: 'warning',
  uri: 'app/src/Legacy.kt',
  line: 14,
  character: 0,
  ...over,
});

describe('describeFinding', () => {
  it('recovers the kind and name from the CLI message', () => {
    expect(describeFinding(finding())).toEqual({ kind: 'class', name: 'LegacyEncoder' });
    expect(describeFinding(finding({ message: "function 'helper' is never used" })))
      .toEqual({ kind: 'function', name: 'helper' });
  });

  it('degrades safely on an unfamiliar message', () => {
    expect(describeFinding(finding({ message: 'something else entirely' })))
      .toEqual({ kind: 'declaration', name: '' });
  });
});

describe('toDiagnostic', () => {
  it('renders as a faded warning attributed to the CLI', () => {
    const d = toDiagnostic(finding());
    expect(d.severity).toBe(DiagnosticSeverity.Warning);
    expect(d.tags).toEqual([DiagnosticTag.Unnecessary]);
    expect(d.source).toBe(DIAGNOSTIC_SOURCE);
    expect(d.range.start.line).toBe(14);
  });

  it('keeps note-level findings quiet as hints', () => {
    expect(toDiagnostic(finding({ level: 'note' })).severity).toBe(DiagnosticSeverity.Hint);
  });

  it('makes the rule id clickable when the run carries a helpUri', () => {
    const withHelp = toDiagnostic(finding({ helpUri: 'https://example.com/dc001' }));
    expect((withHelp.code as any).value).toBe('DC001');
    expect((withHelp.code as any).target.toString()).toContain('dc001');
    expect(toDiagnostic(finding()).code).toBe('DC001'); // plain id without helpUri
  });
});

describe('baseline handling', () => {
  it('builds an entry with a 1-based line, as the CLI stores them', () => {
    expect(toBaselineEntry(finding(), 'app/src/Legacy.kt')).toEqual({
      file: 'app/src/Legacy.kt',
      name: 'LegacyEncoder',
      kind: 'class',
      line: 15,
      rule: 'DC001',
    });
  });

  it('creates the document when there is no baseline yet', () => {
    const out = JSON.parse(appendToBaseline(undefined, toBaselineEntry(finding(), 'a.kt')));
    expect(out.version).toBe(1);
    expect(out.entries).toHaveLength(1);
  });

  it('appends without touching existing entries', () => {
    const first = appendToBaseline(undefined, toBaselineEntry(finding(), 'a.kt'));
    const second = appendToBaseline(first, toBaselineEntry(finding({ message: "fun 'other' is never used" }), 'b.kt'));
    expect(JSON.parse(second).entries.map((e: any) => e.name)).toEqual(['LegacyEncoder', 'other']);
  });

  it('never writes the same entry twice', () => {
    const first = appendToBaseline(undefined, toBaselineEntry(finding(), 'a.kt'));
    const again = appendToBaseline(first, toBaselineEntry(finding(), 'a.kt'));
    expect(JSON.parse(again).entries).toHaveLength(1);
  });

  it('recovers from a corrupt baseline instead of throwing', () => {
    const out = appendToBaseline('{not json', toBaselineEntry(finding(), 'a.kt'));
    expect(JSON.parse(out).entries).toHaveLength(1);
  });

  it('preserves an existing version number', () => {
    const seeded = JSON.stringify({ version: 2, entries: [] });
    expect(JSON.parse(appendToBaseline(seeded, toBaselineEntry(finding(), 'a.kt'))).version).toBe(2);
  });
});

describe('buildIgnoreComment', () => {
  it('copies the declaration indentation', () => {
    expect(buildIgnoreComment('    private val x = 1', 'kept for the migration'))
      .toBe('    // deadcode:ignore(kept for the migration)\n');
  });

  it('handles a top-level declaration', () => {
    expect(buildIgnoreComment('class A', 'public API')).toBe('// deadcode:ignore(public API)\n');
  });

  it('flattens newlines and strips parentheses, which the CLI regex cannot contain', () => {
    expect(buildIgnoreComment('  fun f()', 'used by\nreflection (runtime)'))
      .toBe('  // deadcode:ignore(used by reflection runtime)\n');
  });

  it('produces a directive the CLI regex accepts', () => {
    // the Rust side matches deadcode:ignore(?:\(([^)]*)\))? and refuses an empty reason
    const line = buildIgnoreComment('', 'kept for QA');
    const m = /deadcode:ignore(?:\(([^)]*)\))?/.exec(line);
    expect(m?.[1]).toBe('kept for QA');
  });
});
