const {
  app,
  BrowserWindow,
  shell,
  ipcMain,
  Notification,
  desktopCapturer,
  safeStorage,
} = require('electron');
const path = require('path');
const fs = require('fs');

const isDev = !app.isPackaged;
const PROTOCOL = 'chatsystems';

function authStorePath() {
  return path.join(app.getPath('userData'), 'auth.json');
}

function readAuthStore() {
  try {
    return JSON.parse(fs.readFileSync(authStorePath(), 'utf8'));
  } catch {
    return {};
  }
}

function writeAuthStore(store) {
  try {
    fs.writeFileSync(authStorePath(), JSON.stringify(store), { mode: 0o600 });
  } catch {
    return;
  }
}

function setRefreshToken(url, token) {
  const store = readAuthStore();
  if (token) {
    const buf = safeStorage.isEncryptionAvailable()
      ? safeStorage.encryptString(token)
      : Buffer.from(token, 'utf8');
    store[url] = buf.toString('base64');
  } else {
    delete store[url];
  }
  writeAuthStore(store);
}

function getRefreshToken(url) {
  const enc = readAuthStore()[url];
  if (!enc) return null;
  try {
    const buf = Buffer.from(enc, 'base64');
    return safeStorage.isEncryptionAvailable() ? safeStorage.decryptString(buf) : buf.toString('utf8');
  } catch {
    return null;
  }
}

let mainWindow;
let pendingDeepLink = null;

function sendDeepLink(url) {
  if (!url) return;
  if (mainWindow && !mainWindow.webContents.isLoading()) {
    mainWindow.webContents.send('deep-link', url);
  } else {
    pendingDeepLink = url;
  }
}

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 900,
    minHeight: 600,
    title: 'Chat Systems',
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, 'preload.cjs'),
    },
    backgroundColor: '#0f172a',
    show: false,
  });

  mainWindow.once('ready-to-show', () => {
    mainWindow.show();
  });

  mainWindow.webContents.on('did-finish-load', () => {
    if (pendingDeepLink) {
      mainWindow.webContents.send('deep-link', pendingDeepLink);
      pendingDeepLink = null;
    }
  });

  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    shell.openExternal(url);
    return { action: 'deny' };
  });

  // Huddle screen share: getDisplayMedia() in the renderer needs a main-process
  // handler. On macOS 15+/Windows the OS picker is used; elsewhere we grant the
  // first available source.
  mainWindow.webContents.session.setDisplayMediaRequestHandler(
    (request, callback) => {
      desktopCapturer
        .getSources({ types: ['screen', 'window'] })
        .then((sources) => callback(sources[0] ? { video: sources[0] } : undefined))
        .catch(() => callback(undefined));
    },
    { useSystemPicker: true },
  );

  if (isDev) {
    mainWindow.loadURL('http://localhost:3001');
  } else {
    mainWindow.loadFile(path.join(__dirname, '..', 'dist', 'index.html'));
  }

  if (isDev || process.env.OPEN_DEVTOOLS === '1') {
    mainWindow.webContents.openDevTools({ mode: 'detach' });
  }
}

ipcMain.on('notify', (_event, payload) => {
  if (!Notification.isSupported()) return;
  const { title, body } = payload || {};
  const notification = new Notification({ title: title || 'Chat Systems', body: body || '' });
  notification.on('click', () => {
    if (mainWindow) {
      if (mainWindow.isMinimized()) mainWindow.restore();
      mainWindow.focus();
    }
  });
  notification.show();
});

ipcMain.on('badge-count', (_event, count) => {
  const n = Number(count) || 0;
  if (typeof app.setBadgeCount === 'function') {
    app.setBadgeCount(n);
  }
});

ipcMain.handle('auth:set-refresh', (_event, { url, token }) => {
  setRefreshToken(url, token);
});

ipcMain.handle('auth:clear-refresh', (_event, { url }) => {
  setRefreshToken(url, null);
});

ipcMain.handle('auth:refresh', async (_event, { url }) => {
  const token = getRefreshToken(url);
  if (!token) return null;
  try {
    const res = await fetch(`${url}/api/auth/refresh`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${token}` },
    });
    if (!res.ok) {
      setRefreshToken(url, null);
      return null;
    }
    const data = await res.json();
    if (data?.refresh_token) setRefreshToken(url, data.refresh_token);
    return { access_token: data.access_token, user: data.user, expires_in: data.expires_in };
  } catch {
    return null;
  }
});

const gotSingleInstanceLock = app.requestSingleInstanceLock();
if (!gotSingleInstanceLock) {
  app.quit();
} else {
  app.on('second-instance', (_event, argv) => {
    const url = argv.find((arg) => arg.startsWith(`${PROTOCOL}://`));
    if (url) sendDeepLink(url);
    if (mainWindow) {
      if (mainWindow.isMinimized()) mainWindow.restore();
      mainWindow.focus();
    }
  });

  if (isDev && process.platform === 'win32') {
    app.setAsDefaultProtocolClient(PROTOCOL, process.execPath, [path.resolve(process.argv[1])]);
  } else {
    app.setAsDefaultProtocolClient(PROTOCOL);
  }

  app.on('open-url', (event, url) => {
    event.preventDefault();
    sendDeepLink(url);
  });

  app.whenReady().then(createWindow);
}

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});
