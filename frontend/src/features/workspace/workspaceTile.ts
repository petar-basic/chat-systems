export function workspaceInitials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return '?';
  if (words.length > 1) {
    return words
      .slice(0, 2)
      .map((word) => Array.from(word)[0])
      .join('')
      .toUpperCase();
  }
  const [first = '', second = ''] = Array.from(words[0]);
  return first.toUpperCase() + second.toLowerCase();
}
