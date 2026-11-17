import { describe, it, expect } from 'vitest';
import { levelOf, peaksOf, WAVEFORM_PEAKS } from './waveform';

describe('the waveform of an audio clip', () => {
  it('is a hundred peaks, the loudest at 100', () => {
    const levels = Array.from({ length: 1000 }, (_, i) => (i % 100) / 200);
    const peaks = peaksOf(levels);
    expect(peaks).toHaveLength(WAVEFORM_PEAKS);
    expect(Math.max(...peaks)).toBe(100);
    expect(peaks.every((peak) => Number.isInteger(peak) && peak >= 0 && peak <= 100)).toBe(true);
  });

  it('keeps the loudest moment of each stretch', () => {
    const levels = Array.from({ length: 200 }, () => 0.1);
    levels[3] = 0.4;
    const peaks = peaksOf(levels);
    expect(peaks[1]).toBe(100);
    expect(peaks[0]).toBe(25);
  });

  it('stretches a short clip over all hundred', () => {
    expect(peaksOf([0.5, 1])).toEqual([...Array(50).fill(50), ...Array(50).fill(100)]);
  });

  it('is flat for silence or nothing heard', () => {
    expect(peaksOf([0, 0, 0])).toEqual(Array(100).fill(0));
    expect(peaksOf([])).toEqual(Array(100).fill(0));
  });

  it('reads a level from the analyser around its midpoint', () => {
    expect(levelOf(new Uint8Array([128, 128]))).toBe(0);
    expect(levelOf(new Uint8Array([128, 192, 64]))).toBe(0.5);
    expect(levelOf(new Uint8Array([0]))).toBe(1);
  });
});
