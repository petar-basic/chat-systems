import { describe, it, expect, vi, beforeEach } from 'vitest';

const loaded: string[] = [];
const font = (name: string) => () => {
  loaded.push(name);
  return {};
};
vi.mock('@fontsource/inter/400.css', font('inter'));
vi.mock('@fontsource/inter/500.css', font('inter'));
vi.mock('@fontsource/inter/600.css', font('inter'));
vi.mock('@fontsource/inter/700.css', font('inter'));
vi.mock('@fontsource/atkinson-hyperlegible/400.css', font('atkinson'));
vi.mock('@fontsource/atkinson-hyperlegible/700.css', font('atkinson'));
vi.mock('@fontsource/opendyslexic/400.css', font('opendyslexic'));
vi.mock('@fontsource/opendyslexic/700.css', font('opendyslexic'));

async function freshStore() {
  vi.resetModules();
  return (await import('./text')).useTextStore;
}

describe('useTextStore', () => {
  beforeEach(() => {
    loaded.length = 0;
    localStorage.clear();
    document.documentElement.style.fontSize = '';
    delete document.documentElement.dataset.typeface;
  });

  it('sets the root font size for each step, and leaves the default to the browser', async () => {
    const store = await freshStore();
    for (const [size, root] of [
      ['90', '90%'],
      ['112', '112%'],
      ['125', '125%'],
      ['100', ''],
    ] as const) {
      store.getState().setSize(size);
      expect(document.documentElement.style.fontSize).toBe(root);
    }
    expect(localStorage.getItem('chat_text_size')).toBe('100');
  });

  it('loads a typeface only when it is chosen', async () => {
    const store = await freshStore();
    expect(loaded).toEqual([]);

    store.getState().setTypeface('atkinson_hyperlegible');
    await vi.waitFor(() => expect(loaded).toContain('atkinson'));
    expect(loaded.every((name) => name === 'atkinson')).toBe(true);
    expect(document.documentElement.dataset.typeface).toBe('atkinson_hyperlegible');

    store.getState().setTypeface('system');
    expect(document.documentElement.dataset.typeface).toBeUndefined();
  });

  it('starts in what this browser kept, and ignores a value it does not know', async () => {
    localStorage.setItem('chat_text_size', '125');
    localStorage.setItem('chat_typeface', 'open_dyslexic');
    let store = await freshStore();
    expect(store.getState()).toMatchObject({ size: '125', typeface: 'open_dyslexic' });
    expect(document.documentElement.style.fontSize).toBe('125%');
    await vi.waitFor(() => expect(loaded).toContain('opendyslexic'));

    localStorage.setItem('chat_text_size', '300');
    localStorage.setItem('chat_typeface', 'comic_sans');
    store = await freshStore();
    expect(store.getState()).toMatchObject({ size: '100', typeface: 'system' });
  });
});
