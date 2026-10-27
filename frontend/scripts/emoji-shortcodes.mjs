import data from '@emoji-mart/data' with { type: 'json' };

const rows = [];
for (const [id, emoji] of Object.entries(data.emojis)) {
  rows.push([id, ...emoji.skins.map((skin) => skin.native)]);
}
for (const [alias, id] of Object.entries(data.aliases)) {
  const emoji = data.emojis[id];
  if (emoji) rows.push([alias, ...emoji.skins.map((skin) => skin.native)]);
}
rows.sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
process.stdout.write(rows.map((row) => row.join('\t')).join('\n') + '\n');
