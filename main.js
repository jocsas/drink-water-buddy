const { app, BrowserWindow, ipcMain, Tray, Menu, nativeImage, screen, powerMonitor } = require('electron');
const path = require('path');
const fs = require('fs');
const THEMES = require('./shared/themes');

// ---- Configuration -------------------------------------------------------
const ACTIVE_START_HOUR = 10; // 10:00 IST — first hour reminders may appear
const ACTIVE_END_HOUR = 23;   // 23:00 IST (11 PM) — reminders stop after this
const DEFAULT_INTERVAL_MIN = 45; // normal cadence
const DEFAULT_SNOOZE_MIN = 10;   // "I'll come back in 10 mins"
const GREETING_DELAY_MS = 6000; // first hello after launch, so you can see it work

const WIN_WIDTH = 360;
const WIN_HEIGHT = 430;
const EDGE_MARGIN = 8;
const INTERVAL_OPTIONS = [15, 30, 45, 60, 90, 120];
const SNOOZE_OPTIONS = [5, 10, 15, 20, 30];
const THEME_IDS = Object.keys(THEMES);
// --------------------------------------------------------------------------

let win = null;
let settingsWin = null;
let tray = null;
let ticker = null;
let nextReminderAt = 0; // epoch ms of the next due reminder
let paused = false;
let settings = normalizeSettings({}); // stored per-user, never in the repo

function configureMacMenuBarMode() {
  if (process.platform !== 'darwin') return;

  try {
    if (typeof app.setActivationPolicy === 'function') {
      app.setActivationPolicy('accessory');
    }
    if (app.dock) app.dock.hide();
  } catch (e) {
    /* macOS-only APIs may be unavailable in some Electron runtimes */
  }
}

configureMacMenuBarMode();

// ---- Per-user config (lives in the OS user-data folder, not this repo) ----
function configPath() {
  return path.join(app.getPath('userData'), 'config.json');
}
function loadConfig() {
  try {
    return JSON.parse(fs.readFileSync(configPath(), 'utf8'));
  } catch (e) {
    return {};
  }
}
function saveConfig(cfg) {
  try {
    fs.mkdirSync(path.dirname(configPath()), { recursive: true });
    fs.writeFileSync(configPath(), JSON.stringify(cfg, null, 2));
  } catch (e) {
    console.error('Failed to save config:', e);
  }
}

function clampNumber(value, fallback, min, max) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.max(min, Math.min(max, parsed));
}

function normalizeSettings(raw = {}) {
  const cfg = raw && typeof raw === 'object' ? raw : {};
  const themeId = THEME_IDS.includes(cfg.themeId) ? cfg.themeId : 'default';

  return {
    name: String(cfg.name || '').trim().slice(0, 24),
    intervalMin: clampNumber(cfg.intervalMin, DEFAULT_INTERVAL_MIN, 1, 240),
    snoozeMin: clampNumber(cfg.snoozeMin, DEFAULT_SNOOZE_MIN, 1, 120),
    themeId,
  };
}

function loadSettings() {
  return normalizeSettings(loadConfig());
}

function reminderDelayMs() {
  return settings.intervalMin * 60000;
}

function snoozeDelayMs() {
  return settings.snoozeMin * 60000;
}

function sendSettingsToWindows() {
  if (win && !win.isDestroyed()) {
    win.webContents.send('settings:updated', settings);
  }
  if (settingsWin && !settingsWin.isDestroyed()) {
    settingsWin.webContents.send('settings:updated', settings);
  }
}

function applySettings(nextSettings, opts = {}) {
  const incoming = nextSettings && typeof nextSettings === 'object' && !Array.isArray(nextSettings)
    ? nextSettings
    : {};
  settings = normalizeSettings({ ...settings, ...incoming });
  saveConfig(settings);

  if (opts.reschedule) {
    nextReminderAt = Date.now() + reminderDelayMs();
  }

  updateTrayTooltip();
  if (tray) rebuildTrayMenu();
  sendSettingsToWindows();
  return settings;
}

function themeLabel(themeId = settings.themeId) {
  return THEMES[themeId]?.label || THEMES.default.label;
}

/** Current wall-clock in IST, independent of the machine's own timezone. */
function nowIST() {
  const fmt = new Intl.DateTimeFormat('en-US', {
    timeZone: 'Asia/Kolkata',
    hour12: false,
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
  const parts = Object.fromEntries(
    fmt.formatToParts(new Date()).map((p) => [p.type, p.value])
  );
  let hour = parseInt(parts.hour, 10);
  if (hour === 24) hour = 0; // some ICU builds emit "24" at midnight
  return { hour, minute: parseInt(parts.minute, 10), second: parseInt(parts.second, 10) };
}

function isWithinActiveHours() {
  const { hour } = nowIST();
  return hour >= ACTIVE_START_HOUR && hour < ACTIVE_END_HOUR;
}

const TICK_MS = 30000; // re-check every 30s — short so it survives sleep/wake

/**
 * Runs on a short repeating timer. Because it only ever checks the *current*
 * wall-clock (never a single long countdown), reminders keep working after the
 * laptop sleeps and wakes — a long setTimeout would silently go stale.
 */
function tick() {
  if (paused || !win) return;
  if (win.isVisible()) return; // a reminder is already on screen
  if (!isWithinActiveHours()) return; // outside 10:00–23:00 IST
  if (Date.now() >= nextReminderAt) triggerReminder();
}

function startScheduler() {
  if (ticker) clearInterval(ticker);
  ticker = setInterval(tick, TICK_MS);
}

function positionWindow() {
  const { workArea } = screen.getPrimaryDisplay();
  const x = workArea.x + workArea.width - WIN_WIDTH - EDGE_MARGIN;
  const y = workArea.y + workArea.height - WIN_HEIGHT - EDGE_MARGIN;
  win.setBounds({ x, y, width: WIN_WIDTH, height: WIN_HEIGHT });
}

function triggerReminder() {
  if (paused || !win) return;
  if (!isWithinActiveHours()) return;
  configureMacMenuBarMode();

  // Re-read settings each time so a failed/early startup read can't strand the
  // session with stale config (user data may not be ready the instant
  // auto-start launches at login).
  settings = loadSettings();
  nextReminderAt = Date.now() + reminderDelayMs(); // schedule the next nudge
  updateTrayTooltip();

  positionWindow();
  win.showInactive(); // appear without stealing keyboard focus
  win.setAlwaysOnTop(true, 'screen-saver');
  win.webContents.send('reminder:show', settings);
}

function updateTrayTooltip() {
  if (!tray) return;
  if (paused) {
    tray.setToolTip('Hydrate Buddy — paused');
    return;
  }
  const mins = Math.max(0, Math.round((nextReminderAt - Date.now()) / 60000));
  tray.setToolTip(`Hydrate Buddy — ${themeLabel()} — next nudge in ~${mins} min`);
}

function createWindow() {
  win = new BrowserWindow({
    width: WIN_WIDTH,
    height: WIN_HEIGHT,
    show: false,
    frame: false,
    transparent: true,
    backgroundColor: '#00000000',
    resizable: false,
    movable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    hasShadow: false,
    fullscreenable: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  win.setAlwaysOnTop(true, 'screen-saver');
  win.setVisibleOnAllWorkspaces(true);
  win.loadFile(path.join(__dirname, 'renderer', 'index.html'));

  // Closing the window just hides the pet; quit from the tray.
  win.on('close', (e) => {
    if (!app.isQuiting) {
      e.preventDefault();
      win.hide();
    }
  });
}

function openSettingsWindow() {
  configureMacMenuBarMode();

  if (settingsWin) {
    configureMacMenuBarMode();
    settingsWin.focus();
    configureMacMenuBarMode();
    return;
  }
  settingsWin = new BrowserWindow({
    width: 430,
    height: 470,
    title: 'Hydrate Buddy Settings',
    resizable: false,
    minimizable: false,
    maximizable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  settingsWin.setMenuBarVisibility(false);
  settingsWin.loadFile(path.join(__dirname, 'renderer', 'settings.html'));
  settingsWin.on('show', configureMacMenuBarMode);
  settingsWin.on('focus', configureMacMenuBarMode);
  settingsWin.on('closed', () => {
    settingsWin = null;
  });
}

function createTray() {
  const iconPath = path.join(__dirname, 'assets', 'tray.png');
  let icon = nativeImage.createFromPath(iconPath);
  if (icon.isEmpty()) icon = nativeImage.createEmpty();

  tray = new Tray(icon);
  rebuildTrayMenu();
  tray.setToolTip('Hydrate Buddy');
  tray.on('click', () => triggerReminder());
}

function rebuildTrayMenu() {
  const activeTheme = THEMES[settings.themeId] || THEMES.default;
  const template = [
    { label: `${activeTheme.trayDrinkLabel} 💧`, click: () => triggerReminder() },
    {
      label: 'Settings...',
      click: () => openSettingsWindow(),
    },
    { type: 'separator' },
    {
      label: `Reminder every ${settings.intervalMin} min`,
      submenu: [
        ...INTERVAL_OPTIONS.map((minutes) => ({
          label: `${minutes} min`,
          type: 'radio',
          checked: settings.intervalMin === minutes,
          click: () => applySettings({ intervalMin: minutes }, { reschedule: true }),
        })),
        { type: 'separator' },
        { label: 'Custom...', click: () => openSettingsWindow() },
      ],
    },
    {
      label: `Snooze for ${settings.snoozeMin} min`,
      submenu: [
        ...SNOOZE_OPTIONS.map((minutes) => ({
          label: `${minutes} min`,
          type: 'radio',
          checked: settings.snoozeMin === minutes,
          click: () => applySettings({ snoozeMin: minutes }),
        })),
        { type: 'separator' },
        { label: 'Custom...', click: () => openSettingsWindow() },
      ],
    },
    {
      label: `Theme: ${themeLabel()}`,
      submenu: THEME_IDS.map((themeId) => ({
        label: THEMES[themeId].label,
        type: 'radio',
        checked: settings.themeId === themeId,
        click: () => applySettings({ themeId }),
      })),
    },
    {
      label: settings.name ? `Name: ${settings.name}` : 'Name: not set',
      click: () => openSettingsWindow(),
    },
    {
      label: 'Pause reminders',
      type: 'checkbox',
      checked: paused,
      click: (item) => {
        paused = item.checked;
        if (paused) {
          if (win) win.hide();
        } else {
          nextReminderAt = Date.now() + reminderDelayMs();
        }
        updateTrayTooltip();
      },
    },
  ];

  // In the installed build, offer a native start-at-login toggle.
  if (app.isPackaged) {
    let openAtLogin = false;
    try {
      openAtLogin = app.getLoginItemSettings().openAtLogin;
    } catch (e) {
      /* not supported on this platform */
    }
    template.push({
      label: 'Start at login',
      type: 'checkbox',
      checked: openAtLogin,
      click: (item) => {
        app.setLoginItemSettings({ openAtLogin: item.checked });
      },
    });
  }

  template.push(
    { type: 'separator' },
    {
      label: 'Quit Hydrate Buddy',
      click: () => {
        app.isQuiting = true;
        app.quit();
      },
    }
  );

  tray.setContextMenu(Menu.buildFromTemplate(template));
}

// ---- IPC from the renderer ----------------------------------------------
ipcMain.on('reminder:yes', () => {
  nextReminderAt = Date.now() + reminderDelayMs();
  updateTrayTooltip();
});
ipcMain.on('reminder:snooze', () => {
  nextReminderAt = Date.now() + snoozeDelayMs();
  updateTrayTooltip();
});
ipcMain.on('reminder:hide', () => {
  if (win) win.hide();
});

ipcMain.handle('settings:get', () => settings);
ipcMain.handle('settings:save', (_e, value) => applySettings(value));
ipcMain.on('settings:close', () => {
  if (settingsWin) settingsWin.close();
});

ipcMain.handle('name:get', () => settings.name);
ipcMain.handle('name:save', (_e, value) => {
  const next = applySettings({ name: value });
  return next.name;
});
ipcMain.on('name:close', () => {
  if (settingsWin) settingsWin.close();
});
// --------------------------------------------------------------------------

// Only allow a single running instance.
const gotLock = app.requestSingleInstanceLock();
if (!gotLock) {
  app.quit();
} else {
  app.on('second-instance', () => triggerReminder());
  app.on('activate', configureMacMenuBarMode);
  app.on('browser-window-created', (_event, window) => {
    configureMacMenuBarMode();
    window.on('show', configureMacMenuBarMode);
    window.on('focus', configureMacMenuBarMode);
  });

  app.whenReady().then(() => {
    configureMacMenuBarMode();

    settings = loadSettings();
    createWindow();
    createTray();
    startScheduler();

    // Say hello shortly after launch (within active hours) so you see it works;
    // otherwise the first nudge waits for the next active window.
    nextReminderAt =
      Date.now() + (isWithinActiveHours() ? GREETING_DELAY_MS : reminderDelayMs());
    setTimeout(tick, GREETING_DELAY_MS + 300);

    // Re-check the moment the laptop wakes/unlocks, so a due nudge isn't missed.
    try {
      powerMonitor.on('resume', tick);
      powerMonitor.on('unlock-screen', tick);
    } catch (e) {
      /* powerMonitor unavailable on this platform */
    }
  });
}

// Keep running in the tray even with no visible window.
app.on('window-all-closed', () => {});
