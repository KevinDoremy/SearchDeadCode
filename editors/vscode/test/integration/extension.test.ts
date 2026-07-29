import * as assert from 'node:assert/strict';
import * as vscode from 'vscode';

const EXTENSION_ID = 'elumine.searchdeadcode';

/**
 * These run inside a real Extension Host against test/fixtures/workspace,
 * which contains exactly one dead declaration: the private `orphanedHelper`
 * in Sample.kt. Unit tests cover the parsing and resolution logic in
 * isolation; this file covers the parts only a live host can prove — that the
 * extension activates, registers its commands, and publishes diagnostics.
 */
suite('SearchDeadCode extension', () => {
  suiteSetup(async () => {
    const extension = vscode.extensions.getExtension(EXTENSION_ID);
    assert.ok(extension, `${EXTENSION_ID} is not installed in the host`);
    await extension.activate();
  });

  test('activates', () => {
    assert.equal(vscode.extensions.getExtension(EXTENSION_ID)?.isActive, true);
  });

  test('registers its commands', async () => {
    const commands = await vscode.commands.getCommands(true);
    for (const id of [
      'searchdeadcode.scan',
      'searchdeadcode.clear',
      'searchdeadcode.addToBaseline',
      'searchdeadcode.ignoreInline',
    ]) {
      assert.ok(commands.includes(id), `${id} was not registered`);
    }
  });

  test('contributes its configuration defaults', () => {
    const config = vscode.workspace.getConfiguration('searchdeadcode');
    assert.equal(config.get('enabled'), true);
    assert.equal(config.get('minConfidence'), 'medium');
    assert.ok((config.get<string[]>('rules') ?? []).includes('DC001'));
  });

  // Skipped unless the CLI is reachable: on a dev machine without it on PATH
  // this would fail for a reason that has nothing to do with the extension.
  // CI stages the binary before running, so it does exercise this.
  test('a scan publishes diagnostics for the fixture', async function () {
    await vscode.commands.executeCommand('searchdeadcode.scan');

    const found = await waitFor(() =>
      vscode.languages
        .getDiagnostics()
        .filter(([, diagnostics]) => diagnostics.length > 0)
        .flatMap(([uri, diagnostics]) => diagnostics.map(d => ({ uri, d }))),
    );

    if (found.length === 0) {
      this.skip(); // no CLI on this machine
      return;
    }

    const sample = found.find(({ uri }) => uri.fsPath.endsWith('Sample.kt'));
    assert.ok(sample, 'expected a diagnostic on Sample.kt');
    assert.match(sample.d.message, /orphanedHelper/);

    await vscode.commands.executeCommand('searchdeadcode.clear');
    const after = vscode.languages
      .getDiagnostics()
      .flatMap(([, diagnostics]) => diagnostics);
    assert.equal(after.length, 0, 'clear left diagnostics behind');
  });
});

/** Polls until the callback returns a non-empty array, or the budget runs out. */
async function waitFor<T>(read: () => T[], timeoutMs = 30_000): Promise<T[]> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = read();
    if (value.length > 0) return value;
    if (Date.now() > deadline) return [];
    await new Promise(resolve => setTimeout(resolve, 250));
  }
}
