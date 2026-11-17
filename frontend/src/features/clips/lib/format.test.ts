import { describe, it, expect } from 'vitest';
import { baseType, clipClock, clipFileName, containerOf, recordingType } from './format';

describe('picking what a clip is recorded as', () => {
  it('records MP4 wherever the browser offers it, so it plays everywhere', () => {
    const safari = (type: string) => type.startsWith('video/mp4') || type.startsWith('audio/mp4');
    expect(recordingType('video', safari)).toBe('video/mp4;codecs=avc1.42E01E,mp4a.40.2');
    expect(recordingType('audio', safari)).toBe('audio/mp4;codecs=mp4a.40.2');
  });

  it('falls back to WebM with Opus, which is what Firefox records', () => {
    const firefox = (type: string) => type.includes('webm') && !type.includes('vp9');
    expect(recordingType('video', firefox)).toBe('video/webm;codecs=vp8,opus');
    expect(recordingType('audio', firefox)).toBe('audio/webm;codecs=opus');
  });

  it('says so when the browser records nothing it plays', () => {
    expect(recordingType('video', () => false)).toBeNull();
  });

  it('names the container and stores the type without its codecs', () => {
    expect(baseType('Video/WebM; codecs=vp8,opus')).toBe('video/webm');
    expect(containerOf('audio/mp4;codecs=mp4a.40.2')).toBe('mp4');
    expect(containerOf('video/quicktime')).toBeNull();
  });
});

describe('naming and timing a clip', () => {
  const at = new Date(2026, 10, 17, 9, 5, 7);

  it('names the file after when it was recorded', () => {
    expect(clipFileName('video', 'mp4', at)).toBe('Clip 2026-11-17 09.05.07.mp4');
    expect(clipFileName('audio', 'mp4', at)).toBe('Clip 2026-11-17 09.05.07.m4a');
    expect(clipFileName('audio', 'webm', at)).toBe('Clip 2026-11-17 09.05.07.webm');
  });

  it('shows minutes and seconds', () => {
    expect(clipClock(0)).toBe('0:00');
    expect(clipClock(20_400)).toBe('0:20');
    expect(clipClock(5 * 60 * 1000)).toBe('5:00');
  });
});
