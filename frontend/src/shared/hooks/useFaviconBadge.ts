import { useEffect } from 'react';

function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

function drawFavicon(badge: boolean): string | null {
  const canvas = document.createElement('canvas');
  canvas.width = 32;
  canvas.height = 32;
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;

  ctx.fillStyle = '#7c3aed';
  roundRect(ctx, 0, 0, 32, 32, 8);
  ctx.fill();

  ctx.fillStyle = '#ffffff';
  roundRect(ctx, 6, 7, 20, 14, 4);
  ctx.fill();
  ctx.beginPath();
  ctx.moveTo(11, 19);
  ctx.lineTo(11, 25);
  ctx.lineTo(17, 19);
  ctx.closePath();
  ctx.fill();

  ctx.fillStyle = '#7c3aed';
  for (const cx of [12, 16, 20]) {
    ctx.beginPath();
    ctx.arc(cx, 14, 1.6, 0, Math.PI * 2);
    ctx.fill();
  }

  if (badge) {
    ctx.fillStyle = '#0f172a';
    ctx.beginPath();
    ctx.arc(25, 7, 7, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = '#ef4444';
    ctx.beginPath();
    ctx.arc(25, 7, 5, 0, Math.PI * 2);
    ctx.fill();
  }

  return canvas.toDataURL('image/png');
}

export function useFaviconBadge(hasUnread: boolean) {
  useEffect(() => {
    const link = document.querySelector<HTMLLinkElement>('link[rel="icon"]');
    if (!link) return;
    const href = drawFavicon(hasUnread);
    if (href) link.href = href;
  }, [hasUnread]);
}
