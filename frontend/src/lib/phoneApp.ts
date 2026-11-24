export const inPhoneApp =
  window.location.protocol === 'tauri:' || window.location.hostname === 'tauri.localhost';
