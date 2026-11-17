(() => {
  const canvas = document.getElementById('board');
  const ctx = canvas.getContext('2d');
  const strokes = new Map();
  let frame = 0;

  const alphaOf = (stroke, now) => {
    const into = now - stroke.fade_at;
    return into <= 0 ? 1 : Math.max(0, 1 - into / stroke.fade_ms);
  };

  const drawStroke = (stroke, alpha, width, height) => {
    const [first, ...rest] = stroke.points.map(([x, y]) => [x * width, y * height]);
    if (!first) return;
    ctx.globalAlpha = alpha;
    ctx.strokeStyle = stroke.color;
    ctx.lineWidth = 4;
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    ctx.beginPath();
    ctx.moveTo(first[0], first[1]);
    if (rest.length === 0) ctx.lineTo(first[0] + 0.1, first[1]);
    for (const [x, y] of rest) ctx.lineTo(x, y);
    ctx.stroke();

    ctx.font = '600 13px system-ui, sans-serif';
    ctx.textBaseline = 'middle';
    const labelWidth = ctx.measureText(stroke.name).width + 12;
    const x = Math.max(0, Math.min(first[0] + 8, width - labelWidth));
    const y = Math.max(0, first[1] - 26);
    ctx.fillStyle = stroke.color;
    ctx.beginPath();
    ctx.roundRect(x, y, labelWidth, 20, 5);
    ctx.fill();
    ctx.fillStyle = '#fff';
    ctx.fillText(stroke.name, x + 6, y + 10);
    ctx.globalAlpha = 1;
  };

  const paint = () => {
    frame = 0;
    const dpr = window.devicePixelRatio || 1;
    const width = window.innerWidth;
    const height = window.innerHeight;
    if (canvas.width !== Math.round(width * dpr) || canvas.height !== Math.round(height * dpr)) {
      canvas.width = Math.round(width * dpr);
      canvas.height = Math.round(height * dpr);
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    const now = Date.now();
    for (const [key, stroke] of strokes) {
      const alpha = alphaOf(stroke, now);
      if (alpha <= 0) strokes.delete(key);
      else drawStroke(stroke, alpha, width, height);
    }
    if (strokes.size > 0) frame = requestAnimationFrame(paint);
  };

  const schedule = () => {
    if (!frame) frame = requestAnimationFrame(paint);
  };

  window.addEventListener('resize', schedule);
  window.__TAURI__.event.listen('overlay-stroke', (event) => {
    strokes.set(event.payload.key, event.payload);
    schedule();
  });
})();
