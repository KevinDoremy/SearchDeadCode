import * as vscode from 'vscode';
import { DeadCodeFinding } from './SarifParser';

export const DIAGNOSTIC_SOURCE = 'searchdeadcode';

/** Fingerprint entry appended to `.searchdeadcode-baseline.json`. */
export interface BaselineEntry {
  file: string;
  name: string;
  kind: string;
  line: number;
  rule: string;
}

/**
 * Message shapes the CLI uses, e.g. `class 'LegacyEncoder' is never used`.
 * Used to recover the symbol name and kind for baseline entries; both fall
 * back to safe values when the message does not match.
 */
export function describeFinding(finding: DeadCodeFinding): { name: string; kind: string } {
  const m = /^(\w+)\s+'([^']+)'/.exec(finding.message);
  return { kind: m?.[1] ?? 'declaration', name: m?.[2] ?? '' };
}

export function toBaselineEntry(finding: DeadCodeFinding, relativeFile: string): BaselineEntry {
  const { name, kind } = describeFinding(finding);
  return { file: relativeFile, name, kind, line: finding.line + 1, rule: finding.ruleId };
}

/** Appends to a baseline document, creating the shape when absent. */
export function appendToBaseline(existing: string | undefined, entry: BaselineEntry): string {
  let doc: { version?: number; entries?: BaselineEntry[] } = {};
  if (existing?.trim()) {
    try {
      const parsed = JSON.parse(existing);
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) doc = parsed;
    } catch {
      // unreadable baseline: start a fresh one rather than losing the action
    }
  }
  const entries = Array.isArray(doc.entries) ? doc.entries : [];
  const duplicate = entries.some(
    e => e.file === entry.file && e.name === entry.name && e.rule === entry.rule,
  );
  if (!duplicate) entries.push(entry);
  return `${JSON.stringify({ version: doc.version ?? 1, entries }, null, 2)}\n`;
}

/**
 * `// deadcode:ignore(reason)` line, indented like the declaration it guards.
 * The CLI parses the reason with `\(([^)]*)\)`, so parentheses of any kind
 * would truncate it; they are replaced rather than dropped to keep the text
 * readable.
 */
export function buildIgnoreComment(declarationLine: string, reason: string): string {
  const indent = /^[ \t]*/.exec(declarationLine)?.[0] ?? '';
  const clean = reason
    .replace(/[\r\n]+/g, ' ')
    .replace(/[()]/g, '')
    .replace(/\s+/g, ' ')
    .trim();
  return `${indent}// deadcode:ignore(${clean})\n`;
}

function severityOf(level: DeadCodeFinding['level']): vscode.DiagnosticSeverity {
  // Dead code is never a compile error; note-level findings stay quiet.
  return level === 'note' ? vscode.DiagnosticSeverity.Hint : vscode.DiagnosticSeverity.Warning;
}

export function toDiagnostic(finding: DeadCodeFinding): vscode.Diagnostic {
  const range = new vscode.Range(
    finding.line,
    finding.character,
    finding.line,
    Number.MAX_SAFE_INTEGER,
  );
  const d = new vscode.Diagnostic(range, finding.message, severityOf(finding.level));
  d.source = DIAGNOSTIC_SOURCE;
  d.tags = [vscode.DiagnosticTag.Unnecessary];
  d.code = finding.helpUri
    ? { value: finding.ruleId, target: vscode.Uri.parse(finding.helpUri) }
    : finding.ruleId;
  return d;
}

/**
 * Owns the diagnostics and the quick fixes. Findings are dropped for a file as
 * soon as it is edited: a stale line number is worse than no marker, and it
 * keeps the delete fix from ever cutting the wrong lines.
 */
export class DeadCodeProvider implements vscode.CodeActionProvider, vscode.Disposable {
  static readonly providedCodeActionKinds = [vscode.CodeActionKind.QuickFix];

  private readonly collection = vscode.languages.createDiagnosticCollection(DIAGNOSTIC_SOURCE);
  private readonly byFile = new Map<string, DeadCodeFinding[]>();
  private readonly subs: vscode.Disposable[];

  constructor(private readonly workspaceRoot: () => string | undefined) {
    this.subs = [
      vscode.workspace.onDidChangeTextDocument(e => this.invalidate(e.document.uri)),
    ];
  }

  /** Replaces every finding with a fresh scan result. */
  setFindings(entries: { uri: vscode.Uri; findings: DeadCodeFinding[] }[]): void {
    this.collection.clear();
    this.byFile.clear();
    for (const { uri, findings } of entries) {
      this.byFile.set(uri.toString(), findings);
      this.collection.set(uri, findings.map(toDiagnostic));
    }
  }

  clear(): void {
    this.collection.clear();
    this.byFile.clear();
  }

  private invalidate(uri: vscode.Uri): void {
    const key = uri.toString();
    if (!this.byFile.has(key)) return;
    this.byFile.delete(key);
    this.collection.delete(uri);
  }

  provideCodeActions(
    document: vscode.TextDocument,
    range: vscode.Range | vscode.Selection,
  ): vscode.CodeAction[] {
    const findings = this.byFile.get(document.uri.toString());
    if (!findings) return [];
    const hits = findings.filter(f => f.line === range.start.line);
    if (hits.length === 0) return [];

    const actions: vscode.CodeAction[] = [];
    for (const finding of hits) {
      if (finding.fix) {
        const { name, kind } = describeFinding(finding);
        const action = new vscode.CodeAction(
          name ? `Delete unused ${kind} '${name}'` : 'Delete unused declaration',
          vscode.CodeActionKind.QuickFix,
        );
        const edit = new vscode.WorkspaceEdit();
        edit.delete(
          document.uri,
          new vscode.Range(finding.fix.startLine, 0, finding.fix.endLine + 1, 0),
        );
        action.edit = edit;
        action.isPreferred = true;
        actions.push(action);
      }

      const baseline = new vscode.CodeAction(
        `Add to searchdeadcode baseline (${finding.ruleId})`,
        vscode.CodeActionKind.QuickFix,
      );
      baseline.command = {
        command: 'searchdeadcode.addToBaseline',
        title: 'Add to baseline',
        arguments: [document.uri.toString(), finding],
      };
      actions.push(baseline);

      const ignore = new vscode.CodeAction(
        'Ignore here with a reason',
        vscode.CodeActionKind.QuickFix,
      );
      ignore.command = {
        command: 'searchdeadcode.ignoreInline',
        title: 'Ignore with a reason',
        arguments: [document.uri.toString(), finding.line],
      };
      actions.push(ignore);
    }
    return actions;
  }

  findingsFor(uri: vscode.Uri): DeadCodeFinding[] {
    return this.byFile.get(uri.toString()) ?? [];
  }

  baselineUri(): vscode.Uri | undefined {
    const root = this.workspaceRoot();
    return root ? vscode.Uri.file(`${root}/.searchdeadcode-baseline.json`) : undefined;
  }

  dispose(): void {
    this.collection.dispose();
    for (const s of this.subs) s.dispose();
  }
}
