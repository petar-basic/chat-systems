import { describe, expect, it } from 'vitest';
import { mediaUrl } from './mediaUrl';

const SERVER = 'https://chat.example';
const tokens = { [SERVER]: 'media-1' };

describe('a link to a file on a server signed in with a token', () => {
  it('carries the media token', () => {
    expect(mediaUrl(`${SERVER}/api/v1/files/download/ws/a.png`, tokens)).toBe(
      `${SERVER}/api/v1/files/download/ws/a.png?media=media-1`,
    );
  });

  it('replaces a token it carried before', () => {
    expect(mediaUrl(`${SERVER}/api/v1/files/download/ws/a.png?media=old`, tokens)).toBe(
      `${SERVER}/api/v1/files/download/ws/a.png?media=media-1`,
    );
  });

  it('leaves anything that is not a download alone', () => {
    expect(mediaUrl(`${SERVER}/api/v1/workspaces`, tokens)).toBe(`${SERVER}/api/v1/workspaces`);
    expect(mediaUrl('https://media.giphy.com/a.gif', tokens)).toBe('https://media.giphy.com/a.gif');
    expect(mediaUrl('blob:https://chat.example/1', tokens)).toBe('blob:https://chat.example/1');
  });

  it('leaves a server that uses cookies alone', () => {
    expect(mediaUrl('/api/v1/files/download/ws/a.png', tokens)).toBe('/api/v1/files/download/ws/a.png');
    expect(mediaUrl(`${SERVER}/api/v1/files/download/ws/a.png`, {})).toBe(
      `${SERVER}/api/v1/files/download/ws/a.png`,
    );
  });
});
