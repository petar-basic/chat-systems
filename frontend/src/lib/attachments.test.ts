import { describe, it, expect } from 'vitest';
import { parseAttachment } from './attachments';

describe('parseAttachment', () => {
  it('parses an image upload to an image attachment', () => {
    const a = parseAttachment('[file: photo.png](/api/files/ws1/photo.png)');
    expect(a).toEqual({ filename: 'photo.png', url: '/api/files/ws1/photo.png', isImage: true });
  });

  it('parses a non-image upload as a file (not image)', () => {
    const a = parseAttachment('[file: report.pdf](https://host/files/report.pdf)');
    expect(a?.isImage).toBe(false);
    expect(a?.filename).toBe('report.pdf');
  });

  it('detects images by url extension even with a query string', () => {
    expect(parseAttachment('[file: pic](/x/pic.jpg?v=2)')?.isImage).toBe(true);
  });

  it('rejects unsafe url schemes (javascript:)', () => {
    expect(parseAttachment('[file: x](javascript:alert(1))')).toBeNull();
  });

  it('returns null for ordinary messages and plain links', () => {
    expect(parseAttachment('hello world')).toBeNull();
    expect(parseAttachment('see [docs](https://example.com)')).toBeNull();
  });
});
