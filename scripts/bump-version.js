/**
 * Bump version in all project files.
 * Usage: node scripts/bump-version.js <new-version>
 * Example: node scripts/bump-version.js 1.0.0
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');

const newVersion = process.argv[2];
if (!newVersion || !/^\d+\.\d+\.\d+$/.test(newVersion)) {
  console.error('Usage: node scripts/bump-version.js <semver>');
  console.error('Example: node scripts/bump-version.js 1.0.0');
  process.exit(1);
}

const files = [
  {
    path: 'package.json',
    replace: (content) => content.replace(/"version":\s*"[^"]+"/, `"version": "${newVersion}"`),
  },
  {
    path: 'src-tauri/tauri.conf.json',
    replace: (content) => content.replace(/"version":\s*"[^"]+"/, `"version": "${newVersion}"`),
  },
  {
    path: 'src-tauri/Cargo.toml',
    replace: (content) => content.replace(/^version\s*=\s*"[^"]+"/m, `version = "${newVersion}"`),
  },
];

for (const file of files) {
  const fullPath = resolve(root, file.path);
  try {
    const content = readFileSync(fullPath, 'utf-8');
    const updated = file.replace(content);
    if (content === updated) {
      console.log(`  - ${file.path} (no change)`);
    } else {
      writeFileSync(fullPath, updated);
      console.log(`  ✓ ${file.path} → ${newVersion}`);
    }
  } catch (e) {
    console.error(`  ✗ ${file.path}: ${e.message}`);
  }
}

console.log(`\nVersion bumped to ${newVersion}`);
