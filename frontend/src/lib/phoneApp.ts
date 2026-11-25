export const inPhoneApp =
  window.location.protocol === 'tauri:' || window.location.hostname === 'tauri.localhost';

export const inAndroidApp = inPhoneApp && navigator.userAgent.includes('Android');
