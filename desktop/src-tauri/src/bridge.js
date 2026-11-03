(() => {
  const handlers = new Set();
  const invoke = (command, args) => window.__TAURI_INTERNALS__.invoke(command, args).catch(() => {});

  Object.defineProperty(window, 'chatSystemsDesktop', {
    value: Object.freeze({
      version: 1,
      reportWorkspaces: (workspaces, current) => invoke('report_workspaces', { workspaces, current }),
      notify: (notification) => invoke('notify', { notification }),
      startSso: (invite) => invoke('start_sso', { invite }),
      onCommand: (handler) => {
        handlers.add(handler);
        return () => handlers.delete(handler);
      },
      dispatch: (command) => handlers.forEach((handler) => handler(command)),
    }),
  });
})();
