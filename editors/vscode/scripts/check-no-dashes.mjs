#!/usr/bin/env node
// Scan user-facing copy for em-dash and en-dash. Both are banned in prose.
// Hyphens in compounds are banned too but resist reliable linting (file paths,
// identifiers and proper nouns collide), so only em/en-dash are enforced here.
//
// Backticked spans and fenced blocks are skipped: they may legitimately carry
// the character as data.
//
// Exits 1 when a banned dash is found.

import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const EXTENSION_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');

const TARGETS = ['package.json', 'README.md', 'CHANGELOG.md', 'media/whats-new.json'];

const EM_DASH = '—';
const EN_DASH = '–';

/**
 * Blanks out code spans while preserving newlines, so reported line numbers
 * still line up with the original file.
 */
function stripCode(text) {
  const blank = m => m.replace(/[^\n]/g, ' ');
  return text.replace(/```[\s\S]*?```/g, blank).replace(/`[^`\n]*`/g, blank);
}

let failures = 0;
for (const rel of TARGETS) {
  const abs = join(EXTENSION_ROOT, rel);
  if (!existsSync(abs)) continue;
  const lines = readFileSync(abs, 'utf8').split('\n');
  const stripped = stripCode(readFileSync(abs, 'utf8')).split('\n');
  for (let i = 0; i < lines.length; i++) {
    const line = stripped[i] ?? '';
    if (line.includes(EM_DASH) || line.includes(EN_DASH)) {
      console.error(`${rel}:${i + 1}: ${line.includes(EM_DASH) ? 'em-dash' : 'en-dash'} found`);
      console.error(`  ${lines[i].trim()}`);
      failures++;
    }
  }
}

if (failures > 0) {
  console.error('');
  console.error(`Found ${failures} banned dash(es) in user-facing copy.`);
  console.error('Replace with a period, comma, or colon. Backticked spans are exempt.');
  process.exit(1);
}

console.log('No banned dashes in user-facing copy.');
