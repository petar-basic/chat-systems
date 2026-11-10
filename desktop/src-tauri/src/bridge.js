(() => {
  const handlers = new Set();
  const invoke = (command, args) => window.__TAURI_INTERNALS__.invoke(command, args).catch(() => {});

  const openElsewhere = (event) => {
    if (event.defaultPrevented || event.button > 1) return;
    const link = event.target instanceof Element ? event.target.closest('a[href]') : null;
    if (!link || link.isContentEditable) return;
    const url = new URL(link.href, location.href);
    if (url.origin === location.origin || !['https:', 'http:', 'mailto:'].includes(url.protocol)) return;
    event.preventDefault();
    invoke('open_link', { url: url.href });
  };
  window.addEventListener('click', openElsewhere);
  window.addEventListener('auxclick', openElsewhere);

  Object.defineProperty(window, 'chatSystemsDesktop', {
    value: Object.freeze({
      version: 1,
      reportWorkspaces: (workspaces, current) => invoke('report_workspaces', { workspaces, current }),
      notify: (notification) => invoke('notify', { notification }),
      startSso: (invite) => invoke('start_sso', { invite }),
      startPasskey: () => invoke('start_passkey'),
      reportTheme: (theme) => invoke('report_theme', { theme }),
      reportColours: (colours) => invoke('report_colours', { colours }),
      reportCreation: (allowed) => invoke('report_creation', { allowed }),
      onCommand: (handler) => {
        handlers.add(handler);
        return () => handlers.delete(handler);
      },
      dispatch: (command) => handlers.forEach((handler) => handler(command)),
    }),
  });
})();
