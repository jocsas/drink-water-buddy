import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
const appPath = 'dist/mac-arm64/Hydrate Buddy.app';
const dmgPath = `dist/HydrateBuddy-${packageJson.version}-arm64.dmg`;

function run(command, args) {
  return execFileSync(command, args, {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
}

function fail(message) {
  console.error(`\nRelease verification failed: ${message}\n`);
  process.exit(1);
}

function plistValue(file, key) {
  return run('plutil', ['-extract', key, 'raw', '-o', '-', file]).trim();
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

if (!existsSync(appPath)) {
  fail(`${appPath} does not exist.`);
}

try {
  execFileSync('codesign', ['--verify', '--deep', '--strict', '--verbose=2', appPath], {
    stdio: 'pipe',
  });
} catch (error) {
  fail(`codesign verification failed:\n${error.stderr || error.message}`);
}

const signatureResult = spawnSync('codesign', ['-dv', '--verbose=4', appPath], {
  encoding: 'utf8',
});

if (signatureResult.status !== 0) {
  fail(`unable to inspect signature:\n${signatureResult.stderr || signatureResult.error?.message}`);
}

const signature = `${signatureResult.stdout || ''}\n${signatureResult.stderr || ''}`;

if (!signature.includes('Authority=Developer ID Application:')) {
  fail('app is not signed with a Developer ID Application certificate.');
}

try {
  execFileSync('spctl', ['-a', '-vvv', '-t', 'exec', appPath], { stdio: 'pipe' });
} catch (error) {
  fail(`Gatekeeper assessment failed:\n${error.stderr || error.message}`);
}

try {
  execFileSync('xcrun', ['stapler', 'validate', appPath], { stdio: 'pipe' });
} catch (error) {
  fail(`notarization staple validation failed:\n${error.stderr || error.message}`);
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

if (!existsSync(dmgPath)) {
  fail(`${dmgPath} does not exist.`);
}

console.log('macOS release verification passed.');
