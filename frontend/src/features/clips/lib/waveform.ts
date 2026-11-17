export const WAVEFORM_PEAKS = 100;
const TOP = 100;

export function levelOf(samples: Uint8Array): number {
  let peak = 0;
  for (const sample of samples) peak = Math.max(peak, Math.abs(sample - 128));
  return Math.min(1, peak / 128);
}

export function peaksOf(levels: number[], count = WAVEFORM_PEAKS): number[] {
  if (levels.length === 0) return Array.from({ length: count }, () => 0);
  const buckets = Array.from({ length: count }, (_, i) => {
    const from = Math.floor((i * levels.length) / count);
    const to = Math.max(from + 1, Math.floor(((i + 1) * levels.length) / count));
    return Math.max(...levels.slice(from, to));
  });
  const loudest = Math.max(...buckets);
  if (loudest <= 0) return buckets.map(() => 0);
  return buckets.map((level) => Math.round((Math.max(0, level) / loudest) * TOP));
}
