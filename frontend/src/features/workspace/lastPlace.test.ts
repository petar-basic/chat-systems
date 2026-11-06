import { beforeEach, describe, expect, it } from 'vitest';
import { lastWorkspaceId, placeIn, rememberPlace } from './lastPlace';

describe('lastPlace', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('sends a workspace with nothing remembered to its default place', () => {
    expect(placeIn('w1')).toBe('/app/w1');
    expect(lastWorkspaceId()).toBeUndefined();
  });

  it('remembers where each workspace was left and which workspace was last', () => {
    rememberPlace('w1', '/app/w1/c/dm1');
    rememberPlace('w2', '/app/w2/general');

    expect(placeIn('w1')).toBe('/app/w1/c/dm1');
    expect(placeIn('w2')).toBe('/app/w2/general');
    expect(lastWorkspaceId()).toBe('w2');
  });

  it('ignores a remembered place that belongs to another workspace', () => {
    localStorage.setItem('last-place', JSON.stringify({ places: { w1: '/app/w2/general' } }));

    expect(placeIn('w1')).toBe('/app/w1');
  });

  it('survives storage that is not its own json', () => {
    localStorage.setItem('last-place', 'not json');

    expect(placeIn('w1')).toBe('/app/w1');
    rememberPlace('w1', '/app/w1/threads');
    expect(placeIn('w1')).toBe('/app/w1/threads');
  });
});
