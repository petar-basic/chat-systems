import { describe, expect, it } from 'vitest';
import { linkInside, placeOf } from './appLinks';

const servers = ['https://chat.example', 'https://acme.example/team'];

describe('a link to a server the app is signed in to', () => {
  it('opens the message it points at on that server', () => {
    expect(placeOf('https://chat.example/app/ws-1/ch-2/msg-3', servers)).toEqual({
      server: 'https://chat.example',
      path: '/app/ws-1/ch-2/msg-3',
    });
  });

  it('keeps the query the link carries', () => {
    expect(placeOf('https://chat.example/app/ws-1/ch-2?thread=t-9#top', servers)).toEqual({
      server: 'https://chat.example',
      path: '/app/ws-1/ch-2?thread=t-9',
    });
  });

  it('opens the app for a link to the server itself', () => {
    expect(placeOf('https://chat.example/', servers)).toEqual({
      server: 'https://chat.example',
      path: '/app',
    });
  });

  it('finds a server that lives under a path', () => {
    expect(placeOf('https://acme.example/team/app/ws-1/c/dm-4', servers)).toEqual({
      server: 'https://acme.example/team',
      path: '/app/ws-1/c/dm-4',
    });
  });

  it('leaves pages outside the app to the browser', () => {
    expect(placeOf('https://chat.example/invite/abc', servers)).toBeNull();
    expect(placeOf('https://chat.example/api/v1/files/f-1', servers)).toBeNull();
    expect(placeOf('https://acme.example/other/app/ws-1', servers)).toBeNull();
  });

  it('leaves links to other servers and other schemes alone', () => {
    expect(placeOf('https://elsewhere.example/app/ws-1', servers)).toBeNull();
    expect(placeOf('http://chat.example/app/ws-1', servers)).toBeNull();
    expect(placeOf('mailto:ana@chat.example', servers)).toBeNull();
    expect(placeOf('not a link', servers)).toBeNull();
  });
});

describe('a chat-systems://open link', () => {
  it('carries the web link it opens', () => {
    const inside = linkInside(
      `chat-systems://open?url=${encodeURIComponent('https://chat.example/app/ws-1/ch-2/msg-3')}`,
    );
    expect(inside?.href).toBe('https://chat.example/app/ws-1/ch-2/msg-3');
  });

  it('is ignored when it is for something else or carries no web link', () => {
    expect(linkInside('chat-systems://sso?code=abc&nonce=def')).toBeNull();
    expect(linkInside(`chat-systems://open?url=${encodeURIComponent('javascript:alert(1)')}`)).toBeNull();
    expect(linkInside('chat-systems://open')).toBeNull();
    expect(linkInside('https://chat.example/app')).toBeNull();
  });
});
