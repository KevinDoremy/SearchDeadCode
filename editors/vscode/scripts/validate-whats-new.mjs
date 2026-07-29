#!/usr/bin/env node
// Validate media/whats-new.json before it ships.
//
// This copy is user-facing and often drafted with model help, so the checks
// cover three failure modes: malformed structure, fields that overflow the
// panel, and drafting artifacts that must never reach a user (first-person
// phrasing, placeholders, a bare "claude" reference).
//
// Pass --version <x.y.z> to also assert the file matches the release being cut.
//
// Exits 1 on validation failure, 2 when the file cannot be read at all.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const EXTENSION_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const WHATS_NEW_PATH = join(EXTENSION_ROOT, 'media', 'whats-new.json');
const PACKAGE_JSON = join(EXTENSION_ROOT, 'package.json');

const MAX_HIGHLIGHTS = 3;
const MAX_TITLE_LEN = 120;
const MAX_TAGLINE_LEN = 200;
const MAX_SUMMARY_LEN = 800;
const MAX_DESCRIPTION_LEN = 1200;
const MAX_BODY_LEN = 600;
const VALID_KINDS = new Set(['feature', 'improvement', 'fix', 'note']);
const SEMVER_RE = /^\d+\.\d+\.\d+([.-][0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/;
const URL_RE = /^https?:\/\//;

const LEAK_PATTERNS = [
  { re: /\bI(?:'m| am| will|'ve)\b/i, why: 'first-person phrasing' },
  { re: /\b(?:TODO|TBD|FIXME|XXX)\b/, why: 'placeholder left in copy' },
  { re: /\bclaude\b/i, why: 'bare "claude" reference' },
  { re: /\bas an AI\b/i, why: 'assistant boilerplate' },
  { re: /\blorem ipsum\b/i, why: 'filler text' },
  { re: /<[a-z_]+>/i, why: 'unfilled template placeholder' },
];

const errors = [];
const err = msg => errors.push(msg);

function fatal(msg) {
  process.stderr.write(`validate-whats-new: ERROR ${msg}\n`);
  process.exit(2);
}

let data;
try {
  data = JSON.parse(readFileSync(WHATS_NEW_PATH, 'utf8'));
} catch (e) {
  fatal(`cannot read ${WHATS_NEW_PATH}: ${e.message}`);
}

/** Flags drafting artifacts in any string that reaches a user. */
function checkProse(label, text) {
  for (const { re, why } of LEAK_PATTERNS) {
    if (re.test(text)) err(`${label}: ${why}`);
  }
}

function checkLength(label, text, max) {
  if (typeof text !== 'string') {
    err(`${label} is missing or not a string`);
    return false;
  }
  if (text.trim().length === 0) {
    err(`${label} is empty`);
    return false;
  }
  if (text.length > max) err(`${label} exceeds ${max} chars (got ${text.length})`);
  return true;
}

if (checkLength('version', data.version, 40) && !SEMVER_RE.test(data.version)) {
  err(`version "${data.version}" is not semver`);
}

for (const [field, max] of [
  ['title', MAX_TITLE_LEN],
  ['tagline', MAX_TAGLINE_LEN],
  ['summary', MAX_SUMMARY_LEN],
]) {
  if (checkLength(field, data[field], max)) checkProse(field, data[field]);
}

if (!Array.isArray(data.highlights) || data.highlights.length === 0) {
  err('highlights must be a non-empty array');
} else {
  if (data.highlights.length > MAX_HIGHLIGHTS) {
    err(`${data.highlights.length} highlights, max is ${MAX_HIGHLIGHTS}`);
  }
  data.highlights.forEach((h, i) => {
    const at = `highlights[${i}]`;
    if (checkLength(`${at}.title`, h?.title, MAX_TITLE_LEN)) checkProse(`${at}.title`, h.title);
    if (checkLength(`${at}.description`, h?.description, MAX_DESCRIPTION_LEN)) {
      checkProse(`${at}.description`, h.description);
    }
    if (!VALID_KINDS.has(h?.kind)) {
      err(`${at}.kind "${h?.kind}" is not one of ${[...VALID_KINDS].join(', ')}`);
    }
  });
}

for (const [i, s] of (data.sections ?? []).entries()) {
  const at = `sections[${i}]`;
  if (checkLength(`${at}.title`, s?.title, MAX_TITLE_LEN)) checkProse(`${at}.title`, s.title);
  if (checkLength(`${at}.body`, s?.body, MAX_BODY_LEN)) checkProse(`${at}.body`, s.body);
}

for (const [i, l] of (data.links ?? []).entries()) {
  const at = `links[${i}]`;
  checkLength(`${at}.label`, l?.label, MAX_TITLE_LEN);
  if (!URL_RE.test(l?.url ?? '')) err(`${at}.url is not an http(s) URL`);
}

// The release workflow passes the tag it is publishing. A whats-new left at
// the previous version would ship stale copy to every user.
const versionArg = process.argv.indexOf('--version');
if (versionArg !== -1) {
  const expected = process.argv[versionArg + 1];
  if (!expected) fatal('--version needs a value');
  if (data.version !== expected) {
    err(`whats-new says ${data.version}, release is ${expected}`);
  }
  const pkg = JSON.parse(readFileSync(PACKAGE_JSON, 'utf8'));
  if (pkg.version !== expected) {
    err(`package.json says ${pkg.version}, release is ${expected}`);
  }
}

if (errors.length > 0) {
  for (const e of errors) process.stderr.write(`validate-whats-new: ${e}\n`);
  process.stderr.write(`\n${errors.length} problem(s) in media/whats-new.json\n`);
  process.exit(1);
}

process.stdout.write(`media/whats-new.json is valid (version ${data.version}).\n`);
