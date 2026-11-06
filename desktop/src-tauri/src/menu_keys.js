(() => {
  const menuKeys = (event) => {
    if (!event.ctrlKey || event.altKey || event.metaKey) return null;
    if (event.shiftKey) return event.code === 'KeyN' ? 'add-server' : null;
    const digit = /^Digit([1-9])$/.exec(event.code);
    return digit ? { rail: Number(digit[1]) - 1 } : null;
  };

  window.addEventListener(
    'keydown',
    (event) => {
      const keys = menuKeys(event);
      if (!keys) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      if (event.repeat) return;
      window.__TAURI_INTERNALS__.invoke('press_menu_keys', { keys }).catch(() => {});
    },
    true,
  );
})();
