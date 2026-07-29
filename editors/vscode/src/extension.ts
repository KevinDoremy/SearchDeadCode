import * as vscode from 'vscode';
import { existsSync } from 'node:fs';
import { relative } from 'node:path';
import { DeadCodeFinding, resolveFindingPath } from './SarifParser';
import { MIN_VERSION, resolveBinary } from './SearchDeadCodeBinary';
import { runScan, ScanError } from './SearchDeadCodeService';
import {
  DeadCodeProvider,
  appendToBaseline,
  buildIgnoreComment,
  toBaselineEntry,
} from './DeadCodeProvider';

const INSTALL_HOMEBREW = 'Install with Homebrew';
const INSTALL_CARGO = 'Install with cargo';
const OPEN_RELEASES = 'Open releases';
const SET_PATH = 'Set path…';

let output: vscode.OutputChannel;

function config() {
  return vscode.workspace.getConfiguration('searchdeadcode');
}

function workspaceRoot(): string | undefined {
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}

async function offerInstall(): Promise<void> {
  const choice = await vscode.window.showWarningMessage(
    'searchdeadcode was not found. Install the CLI to scan this workspace for dead code.',
    INSTALL_HOMEBREW,
    INSTALL_CARGO,
    OPEN_RELEASES,
    SET_PATH,
  );
  if (choice === INSTALL_HOMEBREW) {
    const terminal = vscode.window.createTerminal('searchdeadcode');
    terminal.show();
    terminal.sendText('brew install KevinDoremy/tap/searchdeadcode');
  } else if (choice === INSTALL_CARGO) {
    const terminal = vscode.window.createTerminal('searchdeadcode');
    terminal.show();
    terminal.sendText('cargo install searchdeadcode');
  } else if (choice === OPEN_RELEASES) {
    await vscode.env.openExternal(
      vscode.Uri.parse('https://github.com/KevinDoremy/searchdeadcode/releases'),
    );
  } else if (choice === SET_PATH) {
    await vscode.commands.executeCommand('workbench.action.openSettings', 'searchdeadcode.path');
  }
}

/** Groups findings by the file they belong to, resolving CLI paths. */
function groupByFile(root: string, findings: DeadCodeFinding[]) {
  const byPath = new Map<string, DeadCodeFinding[]>();
  for (const finding of findings) {
    const absolute = resolveFindingPath(root, finding.uri, existsSync);
    const list = byPath.get(absolute);
    if (list) list.push(finding);
    else byPath.set(absolute, [finding]);
  }
  return [...byPath].map(([path, list]) => ({ uri: vscode.Uri.file(path), findings: list }));
}

async function scan(provider: DeadCodeProvider): Promise<void> {
  const root = workspaceRoot();
  if (!root) {
    void vscode.window.showWarningMessage('Open a folder before scanning for dead code.');
    return;
  }

  const cfg = config();
  const binary = await resolveBinary({ configuredPath: cfg.get<string>('path', '') });
  if (!binary) {
    await offerInstall();
    return;
  }
  if (!binary.supported) {
    void vscode.window.showWarningMessage(
      `searchdeadcode ${binary.version} is too old for this extension (needs ${MIN_VERSION} or newer).`,
    );
    return;
  }

  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: 'Scanning for dead code…',
      cancellable: true,
    },
    async (_progress, token) => {
      const controller = new AbortController();
      token.onCancellationRequested(() => controller.abort());
      try {
        const findings = await runScan(
          {
            binaryPath: binary.path,
            workspaceRoot: root,
            minConfidence: cfg.get<string>('minConfidence', 'medium'),
            rules: cfg.get<string[]>('rules', []),
            exclude: cfg.get<string[]>('exclude', []),
            extraArgs: cfg.get<string[]>('extraArgs', []),
          },
          controller.signal,
        );
        provider.setFindings(groupByFile(root, findings));
        const message =
          findings.length === 0
            ? 'No dead code found.'
            : `Found ${findings.length} dead code ${findings.length === 1 ? 'finding' : 'findings'}.`;
        void vscode.window.showInformationMessage(message);
      } catch (e) {
        if (e instanceof ScanError) {
          if (e.detail) output.appendLine(e.detail);
          if (!/cancelled/i.test(e.message)) {
            void vscode.window.showErrorMessage(`searchdeadcode: ${e.message}`);
          }
        } else {
          output.appendLine(String(e));
          void vscode.window.showErrorMessage('searchdeadcode: the scan failed, see the output channel.');
        }
      }
    },
  );
}

async function addToBaseline(provider: DeadCodeProvider, uriString: string, finding: DeadCodeFinding): Promise<void> {
  const root = workspaceRoot();
  const baselineUri = provider.baselineUri();
  if (!root || !baselineUri) return;

  let existing: string | undefined;
  try {
    existing = new TextDecoder().decode(await vscode.workspace.fs.readFile(baselineUri));
  } catch {
    existing = undefined; // no baseline yet
  }
  const relativeFile = relative(root, vscode.Uri.parse(uriString).fsPath);
  const next = appendToBaseline(existing, toBaselineEntry(finding, relativeFile));
  await vscode.workspace.fs.writeFile(baselineUri, new TextEncoder().encode(next));
  void vscode.window.showInformationMessage(
    'Added to .searchdeadcode-baseline.json. Rescan to refresh the findings.',
  );
}

async function ignoreInline(uriString: string, line: number): Promise<void> {
  const reason = await vscode.window.showInputBox({
    title: 'Ignore this finding',
    prompt: 'searchdeadcode requires a reason for every inline ignore',
    placeHolder: 'why this declaration must stay',
    validateInput: value => (value.trim().length === 0 ? 'A reason is required.' : undefined),
  });
  if (!reason) return;

  const document = await vscode.workspace.openTextDocument(vscode.Uri.parse(uriString));
  const declarationLine = document.lineAt(line).text;
  const edit = new vscode.WorkspaceEdit();
  edit.insert(document.uri, new vscode.Position(line, 0), buildIgnoreComment(declarationLine, reason));
  await vscode.workspace.applyEdit(edit);
}

export function activate(context: vscode.ExtensionContext): void {
  output = vscode.window.createOutputChannel('SearchDeadCode');
  const provider = new DeadCodeProvider(workspaceRoot);

  context.subscriptions.push(
    output,
    provider,
    vscode.languages.registerCodeActionsProvider(
      [{ language: 'kotlin' }, { language: 'java' }],
      provider,
      { providedCodeActionKinds: DeadCodeProvider.providedCodeActionKinds },
    ),
    vscode.commands.registerCommand('searchdeadcode.scan', () => {
      if (!config().get<boolean>('enabled', true)) {
        void vscode.window.showInformationMessage('SearchDeadCode is disabled in settings.');
        return;
      }
      return scan(provider);
    }),
    vscode.commands.registerCommand('searchdeadcode.clear', () => provider.clear()),
    vscode.commands.registerCommand(
      'searchdeadcode.addToBaseline',
      (uriString: string, finding: DeadCodeFinding) => addToBaseline(provider, uriString, finding),
    ),
    vscode.commands.registerCommand(
      'searchdeadcode.ignoreInline',
      (uriString: string, line: number) => ignoreInline(uriString, line),
    ),
  );
}

export function deactivate(): void {
  // subscriptions handle teardown
}
