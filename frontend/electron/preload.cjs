const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electronAPI', {
  platform: process.platform,
  isElectron: true,
  send: (channel, data) => {
    const validChannels = ['notify', 'badge-count'];
    if (validChannels.includes(channel)) {
      ipcRenderer.send(channel, data);
    }
  },
  on: (channel, func) => {
    const validChannels = ['deep-link'];
    if (validChannels.includes(channel)) {
      ipcRenderer.on(channel, (event, ...args) => func(...args));
    }
  },
  auth: {
    setRefresh: (url, token) => ipcRenderer.invoke('auth:set-refresh', { url, token }),
    clearRefresh: (url) => ipcRenderer.invoke('auth:clear-refresh', { url }),
    refresh: (url) => ipcRenderer.invoke('auth:refresh', { url }),
  },
});
