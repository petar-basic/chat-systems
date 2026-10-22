import { describe, it, expect } from 'vitest';
import { formatBytes } from './fileSize';

describe('formatBytes', () => {
  it('picks a readable unit', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(25 * 1024 * 1024)).toBe('25 MB');
    expect(formatBytes(100 * 1024 * 1024 * 1024)).toBe('100 GB');
  });
});
