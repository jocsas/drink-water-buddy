// Exposes the same `window.hydrate` API the renderer expects, but routed over
// Tauri commands/events instead of Electron's ipcRenderer.
(function () {
  const { invoke } = window.__TAURI__.core;
  const { listen } = window.__TAURI__.event;

  const api = {
    onShow: (cb) =>
      listen('reminder:show', (event) => cb(event.payload || {})),
    onSettingsUpdated: (cb) =>
      listen('settings:updated', (event) => cb(event.payload || {})),

    yes: () => invoke('reminder_yes'),
    snooze: () => invoke('reminder_snooze'),
    hide: () => invoke('reminder_hide'),

    getSettings: () => invoke('settings_get'),
    saveSettings: (value) => invoke('settings_save', { value }),
    closeSettingsWindow: () => invoke('settings_close'),
  };

  window.hydrate = api;
})();
