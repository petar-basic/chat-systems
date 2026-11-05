(function () {
  var stored = null;
  try {
    stored = localStorage.getItem('chat_theme');
  } catch (e) {
    stored = null;
  }
  var resolved =
    stored === 'light' || stored === 'dark'
      ? stored
      : window.matchMedia && window.matchMedia('(prefers-color-scheme: light)').matches
        ? 'light'
        : 'dark';
  document.documentElement.dataset.theme = resolved;
  var meta = document.querySelector('meta[name="theme-color"]');
  if (meta) meta.setAttribute('content', resolved === 'light' ? '#eef1f6' : '#0f172a');

  try {
    var size = localStorage.getItem('chat_text_size');
    if (size === '90' || size === '112' || size === '125') document.documentElement.style.fontSize = size + '%';
    var typeface = localStorage.getItem('chat_typeface');
    if (typeface === 'inter' || typeface === 'atkinson_hyperlegible' || typeface === 'open_dyslexic')
      document.documentElement.dataset.typeface = typeface;
  } catch (e) {
    size = null;
  }

  window.__installPrompt = null;
  window.addEventListener('beforeinstallprompt', function (event) {
    event.preventDefault();
    window.__installPrompt = event;
    window.dispatchEvent(new Event('installpromptchange'));
  });
  window.addEventListener('appinstalled', function () {
    window.__installPrompt = null;
    window.dispatchEvent(new Event('installpromptchange'));
  });
})();
