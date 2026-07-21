import { spawn } from 'node:child_process';
import { existsSync, statSync, watch } from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const rootDir = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const electronBin = path.join(
  rootDir,
  'node_modules',
  '.bin',
  process.platform === 'win32' ? 'electron.cmd' : 'electron'
);

const watchedPaths = [
  'main.js',
  'preload.js',
  'package.json',
  'renderer',
  'assets',
].map((entry) => path.join(rootDir, entry));

let child = null;
let restartTimer = null;
let stopping = false;

function startApp() {
  if (!existsSync(electronBin)) {
    console.error('Electron is not installed. Run `npm install` first.');
    process.exitCode = 1;
    return;
  }

  console.log('Starting Hydrate Buddy dev app...');
  child = spawn(electronBin, ['.'], {
    cwd: rootDir,
    stdio: 'inherit',
    env: {
      ...process.env,
      HYDRATE_BUDDY_DEV: '1',
      ELECTRON_ENABLE_LOGGING: '1',
    },
  });

  child.on('exit', (code, signal) => {
    child = null;
    if (!stopping && code !== 0 && signal !== 'SIGTERM') {
      console.log(`Electron exited with code ${code ?? signal}. Waiting for changes...`);
    }
  });
}

function restartApp(reason) {
  clearTimeout(restartTimer);
  restartTimer = setTimeout(() => {
    console.log(`Change detected${reason ? `: ${reason}` : ''}. Restarting...`);
    if (!child) {
      startApp();
      return;
    }
    const previousChild = child;
    previousChild.once('exit', startApp);
    previousChild.kill('SIGTERM');
  }, 150);
}

function watchPath(targetPath) {
  if (!existsSync(targetPath)) return;
  const isDirectory = statSync(targetPath).isDirectory();

  const watcher = watch(
    targetPath,
    { recursive: process.platform === 'darwin' && isDirectory },
    (eventType, filename) => restartApp(filename || eventType)
  );
  watcher.on('error', (error) => {
    console.error(`Watcher error for ${path.relative(rootDir, targetPath)}:`, error.message);
  });
}

for (const targetPath of watchedPaths) {
  watchPath(targetPath);
}

process.on('SIGINT', () => {
  stopping = true;
  if (child) child.kill('SIGTERM');
  process.exit(0);
});

process.on('SIGTERM', () => {
  stopping = true;
  if (child) child.kill('SIGTERM');
  process.exit(0);
});

startApp();
