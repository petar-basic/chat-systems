const SIDE = 88;

const drawn = new Map<string, Promise<string | null>>();

async function draw(url: string): Promise<string | null> {
  try {
    const response = await fetch(url, { credentials: 'include' });
    if (!response.ok) return null;
    const bitmap = await createImageBitmap(await response.blob());
    const canvas = document.createElement('canvas');
    canvas.width = SIDE;
    canvas.height = SIDE;
    const context = canvas.getContext('2d');
    if (!context) return null;
    const scale = Math.max(SIDE / bitmap.width, SIDE / bitmap.height);
    const width = bitmap.width * scale;
    const height = bitmap.height * scale;
    context.drawImage(bitmap, (SIDE - width) / 2, (SIDE - height) / 2, width, height);
    bitmap.close();
    return canvas.toDataURL('image/png');
  } catch {
    return null;
  }
}

export function railIcon(url: string): Promise<string | null> {
  const resolved = new URL(url, window.location.origin);
  if (resolved.origin !== window.location.origin) return Promise.resolve(null);
  const key = resolved.href;
  const known = drawn.get(key);
  if (known) return known;
  const icon = draw(key).then((data) => {
    if (!data) drawn.delete(key);
    return data;
  });
  drawn.set(key, icon);
  return icon;
}
