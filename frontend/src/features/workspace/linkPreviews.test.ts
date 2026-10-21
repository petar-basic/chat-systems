import { describe, expect, it } from 'vitest';
import { linkPreviewsEnabled } from './linkPreviews';

describe('linkPreviewsEnabled', () => {
  it('is on unless a workspace turned it off', () => {
    expect(linkPreviewsEnabled({})).toBe(true);
    expect(linkPreviewsEnabled(null)).toBe(true);
    expect(linkPreviewsEnabled({ link_previews: true })).toBe(true);
    expect(linkPreviewsEnabled({ link_previews: false })).toBe(false);
  });
});
