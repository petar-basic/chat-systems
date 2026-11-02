function initials(text) {
  return text
    .split(/[\s._-]+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((word) => word[0].toUpperCase())
    .join('');
}

function hostWord(url) {
  const words = new URL(url).hostname.split('.').filter((w) => !['www', 'chat'].includes(w));
  return words[0] ?? new URL(url).hostname;
}

export function label(server, servers) {
  const shared = servers.filter((s) => s.name === server.name).length > 1;
  return document.createTextNode(shared ? hostWord(server.url).slice(0, 2).toUpperCase() : initials(server.name));
}
