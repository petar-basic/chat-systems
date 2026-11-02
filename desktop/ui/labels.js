export function initials(text) {
  return text
    .split(/[\s._-]+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((word) => word[0].toUpperCase())
    .join('');
}

export function colorFor(id) {
  let hash = 0;
  for (const c of id) hash = (hash * 31 + c.charCodeAt(0)) >>> 0;
  return `hsl(${hash % 360} 55% 45%)`;
}

function hostWord(url) {
  const words = new URL(url).hostname.split('.').filter((w) => !['www', 'chat'].includes(w));
  return words[0] ?? new URL(url).hostname;
}

export function label(server, servers) {
  const shared = servers.filter((s) => s.name === server.name).length > 1;
  return shared ? hostWord(server.url).slice(0, 2).toUpperCase() : initials(server.name);
}
