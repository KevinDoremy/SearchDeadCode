/**
 * SARIF 2.1.0 → findings. Pure module: no vscode, no fs, fully unit tested.
 *
 * Written against real output of the searchdeadcode CLI, which varies by
 * version: 0.10 emits neither `partialFingerprints` nor `fixes[]`, newer
 * versions add both. Everything past the required core is treated as
 * optional so the bridge degrades instead of breaking.
 */

export interface DeadCodeFinding {
  ruleId: string;
  message: string;
  /** SARIF level, defaulted to 'warning' when the run omits it. */
  level: 'error' | 'warning' | 'note';
  /** Path exactly as the CLI reported it (resolution is the caller's job). */
  uri: string;
  /** 0-based, VS Code convention. */
  line: number;
  character: number;
  helpUri?: string;
  fingerprint?: string;
  /** 0-based inclusive line range the CLI says is safe to delete. */
  fix?: { startLine: number; endLine: number };
}

export class SarifParseError extends Error {}

const LEVELS = new Set(['error', 'warning', 'note']);

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

/**
 * @param text raw SARIF document
 * @param ruleAllowlist rule IDs to keep; empty or undefined keeps everything
 */
export function parseSarif(text: string, ruleAllowlist?: readonly string[]): DeadCodeFinding[] {
  let doc: unknown;
  try {
    doc = JSON.parse(text);
  } catch (e) {
    throw new SarifParseError(`not valid JSON: ${(e as Error).message}`);
  }

  const root = asRecord(doc);
  if (!root) throw new SarifParseError('root is not an object');
  if (typeof root.version !== 'string' || !root.version.startsWith('2.1')) {
    throw new SarifParseError(`unsupported SARIF version: ${String(root.version)}`);
  }
  const runs = Array.isArray(root.runs) ? root.runs : undefined;
  if (!runs) throw new SarifParseError('missing runs[]');

  const allow = ruleAllowlist && ruleAllowlist.length > 0 ? new Set(ruleAllowlist) : undefined;
  const findings: DeadCodeFinding[] = [];

  for (const rawRun of runs) {
    const run = asRecord(rawRun);
    if (!run) continue;

    // rules[] carries helpUri and the default level; both are optional
    const helpUris = new Map<string, string>();
    const defaultLevels = new Map<string, string>();
    const driver = asRecord(asRecord(run.tool)?.driver);
    for (const rawRule of Array.isArray(driver?.rules) ? driver!.rules : []) {
      const rule = asRecord(rawRule);
      const id = rule?.id;
      if (typeof id !== 'string') continue;
      if (typeof rule!.helpUri === 'string') helpUris.set(id, rule!.helpUri);
      const level = asRecord(rule!.defaultConfiguration)?.level;
      if (typeof level === 'string') defaultLevels.set(id, level);
    }

    for (const rawResult of Array.isArray(run.results) ? run.results : []) {
      const result = asRecord(rawResult);
      if (!result) continue;
      const ruleId = typeof result.ruleId === 'string' ? result.ruleId : undefined;
      if (!ruleId) continue;
      if (allow && !allow.has(ruleId)) continue;

      const physical = asRecord(asRecord(Array.isArray(result.locations) ? result.locations[0] : undefined)?.physicalLocation);
      const uri = asRecord(physical?.artifactLocation)?.uri;
      const region = asRecord(physical?.region);
      if (typeof uri !== 'string' || !region) continue;

      const startLine = typeof region.startLine === 'number' ? region.startLine : undefined;
      if (startLine === undefined) continue;
      const startColumn = typeof region.startColumn === 'number' ? region.startColumn : 1;

      const message = asRecord(result.message)?.text;
      const rawLevel = typeof result.level === 'string' ? result.level : defaultLevels.get(ruleId);
      const level = (rawLevel && LEVELS.has(rawLevel) ? rawLevel : 'warning') as DeadCodeFinding['level'];

      const fingerprints = asRecord(result.partialFingerprints);
      const fingerprint = fingerprints
        ? Object.values(fingerprints).find((v): v is string => typeof v === 'string')
        : undefined;

      // fixes[]: only a whole-line deletion is understood; anything else is ignored
      let fix: DeadCodeFinding['fix'];
      const firstFix = asRecord(Array.isArray(result.fixes) ? result.fixes[0] : undefined);
      const change = asRecord(Array.isArray(firstFix?.artifactChanges) ? firstFix!.artifactChanges[0] : undefined);
      const replacement = asRecord(Array.isArray(change?.replacements) ? change!.replacements[0] : undefined);
      const deleted = asRecord(replacement?.deletedRegion);
      const inserted = asRecord(replacement?.insertedContent);
      const insertedText = inserted?.text;
      if (
        deleted &&
        typeof deleted.startLine === 'number' &&
        typeof deleted.endLine === 'number' &&
        (insertedText === undefined || insertedText === '')
      ) {
        fix = { startLine: deleted.startLine - 1, endLine: deleted.endLine - 1 };
      }

      findings.push({
        ruleId,
        message: typeof message === 'string' ? message : `${ruleId} reported here`,
        level,
        uri,
        line: Math.max(startLine - 1, 0),
        character: Math.max(startColumn - 1, 0),
        helpUri: helpUris.get(ruleId),
        fingerprint,
        fix,
      });
    }
  }

  return findings;
}

/**
 * Resolves a SARIF uri against the scanned root.
 *
 * The CLI has reported paths relative to the PARENT of the scanned directory
 * (`myProject/src/Main.kt` for a scan of `myProject`), so a naive join
 * produces `myProject/myProject/src/Main.kt`. Strips the root's own basename
 * when the uri starts with it; `exists` lets the caller confirm.
 */
export function resolveFindingPath(
  workspaceRoot: string,
  uri: string,
  exists: (absolutePath: string) => boolean,
): string {
  if (uri.startsWith('/')) return uri;
  const join = (a: string, b: string) => `${a.replace(/\/+$/, '')}/${b.replace(/^\/+/, '')}`;
  const direct = join(workspaceRoot, uri);
  if (exists(direct)) return direct;

  const rootName = workspaceRoot.replace(/\/+$/, '').split('/').pop() ?? '';
  if (rootName && (uri === rootName || uri.startsWith(`${rootName}/`))) {
    const stripped = join(workspaceRoot, uri.slice(rootName.length));
    if (exists(stripped)) return stripped;
  }
  // last resort: report against the workspace so the finding is never dropped
  return direct;
}
