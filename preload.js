const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('hydrate', {
  // main -> renderer: a reminder should be shown now (payload: settings)
  onShow: (cb) => ipcRenderer.on('reminder:show', (_e, payload) => cb(payload || {})),
  onSettingsUpdated: (cb) =>
    ipcRenderer.on('settings:updated', (_e, payload) => cb(payload || {})),
  // renderer -> main
  yes: () => ipcRenderer.send('reminder:yes'),
  snooze: () => ipcRenderer.send('reminder:snooze'),
  hide: () => ipcRenderer.send('reminder:hide'),
  // app settings
  getSettings: () => ipcRenderer.invoke('settings:get'),
  saveSettings: (value) => ipcRenderer.invoke('settings:save', value),
  closeSettingsWindow: () => ipcRenderer.send('settings:close'),
  // backwards-compatible name helpers
  getName: () => ipcRenderer.invoke('name:get'),
  saveName: (value) => ipcRenderer.invoke('name:save', value),
  closeNameWindow: () => ipcRenderer.send('name:close'),
});
