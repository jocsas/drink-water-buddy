import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const appPathCandidates = [
  'dist/mac-arm64/Hydrate Buddy.app',
  'dist/mac-universal/Hydrate Buddy.app',
  'dist/mac/Hydrate Buddy.app',
];
const appPath = appPathCandidates.find((candidate) => existsSync(candidate));

function fail(message) {
  console.error(`\nMenu bar verification failed: ${message}\n`);
  process.exit(1);
}

function plistValue(file, key) {
  return execFileSync('plutil', ['-extract', key, 'raw', '-o', '-', file], {
    encoding: 'utf8',
  }).trim();
}

function findHelperPlists(dir) {
  const results = [];
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    const stat = statSync(path);
    if (stat.isDirectory()) {
      if (path.endsWith('.app')) {
        const plist = join(path, 'Contents', 'Info.plist');
        if (existsSync(plist)) results.push(plist);
      } else {
        results.push(...findHelperPlists(path));
      }
    }
  }
  return results;
}

if (!appPath) {
  fail(`no packaged app was found. Checked: ${appPathCandidates.join(', ')}`);
}

const mainPlist = join(appPath, 'Contents', 'Info.plist');
if (plistValue(mainPlist, 'LSUIElement') !== 'true') {
  fail('main app is not configured as LSUIElement=true, so it can appear in the Dock.');
}

for (const plist of findHelperPlists(join(appPath, 'Contents', 'Frameworks'))) {
  if (plistValue(plist, 'LSUIElement') !== 'true') {
    fail(`${plist} is not configured as LSUIElement=true.`);
  }
}

console.log('Menu bar verification passed.');
