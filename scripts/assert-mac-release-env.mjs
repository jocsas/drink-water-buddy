import { execFileSync } from 'node:child_process';

function fail(message) {
  console.error(`\nRelease build blocked: ${message}\n`);
  process.exit(1);
}

function hasAll(names) {
  return names.every((name) => Boolean(process.env[name]));
}

if (process.platform !== 'darwin') {
  fail('macOS releases must be built on macOS so the app can be signed and notarized.');
}

const hasCertificateEnv = Boolean(process.env.CSC_LINK && process.env.CSC_KEY_PASSWORD);

let identities = '';
try {
  identities = execFileSync('security', ['find-identity', '-v', '-p', 'codesigning'], {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
} catch (error) {
  identities = `${error.stdout || ''}\n${error.stderr || ''}`;
}

if (!hasCertificateEnv && !identities.includes('Developer ID Application:')) {
  fail(
    'no valid "Developer ID Application" signing identity was found. Install a Developer ID Application certificate in Keychain, or set CSC_LINK and CSC_KEY_PASSWORD.'
  );
}

const hasApiKey = hasAll(['APPLE_API_KEY', 'APPLE_API_KEY_ID', 'APPLE_API_ISSUER']);
const hasAppleId = hasAll(['APPLE_ID', 'APPLE_APP_SPECIFIC_PASSWORD', 'APPLE_TEAM_ID']);
const hasKeychainProfile = Boolean(process.env.APPLE_KEYCHAIN_PROFILE);

if (!hasApiKey && !hasAppleId && !hasKeychainProfile) {
  fail(
    'notarization credentials are missing. Set APPLE_API_KEY + APPLE_API_KEY_ID + APPLE_API_ISSUER, or APPLE_ID + APPLE_APP_SPECIFIC_PASSWORD + APPLE_TEAM_ID, or APPLE_KEYCHAIN_PROFILE.'
  );
}

console.log('macOS release prerequisites look OK.');
