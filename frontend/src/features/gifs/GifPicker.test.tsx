import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useRef } from 'react';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { GifPicker } from './GifPicker';
import { GIF_PAGE } from './giphy';
import { SETTINGS, answer, giphyPage } from '@/test/giphy';

const onPick = vi.fn();
const fetch = vi.fn();

function Picker({ initialQuery }: { initialQuery?: string }) {
  const anchor = useRef<HTMLButtonElement>(null);
  return (
    <>
      <button ref={anchor}>anchor</button>
      <GifPicker
        anchorRef={anchor}
        settings={SETTINGS}
        initialQuery={initialQuery}
        onPick={onPick}
        onClose={vi.fn()}
      />
    </>
  );
}

function open(initialQuery?: string) {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <Picker initialQuery={initialQuery} />
    </QueryClientProvider>,
  );
  return screen.getByTestId('gif-picker');
}

function askedFor(call: number) {
  return new URL(fetch.mock.calls[call]?.[0] as string);
}

beforeEach(() => {
  onPick.mockReset();
  fetch.mockReset();
  vi.stubGlobal('fetch', fetch);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('GifPicker', () => {
  it('opens on trending GIFs with GIPHY credited', async () => {
    fetch.mockImplementation(() => answer(giphyPage(['a', 'b'])));
    const picker = open();

    await waitFor(() => expect(within(picker).getAllByTestId('gif-choice')).toHaveLength(2));
    expect(askedFor(0).pathname).toBe('/v1/gifs/trending');
    expect(within(picker).getByRole('list', { name: 'Trending GIFs' })).toBeInTheDocument();
    expect(within(picker).getByTestId('gif-attribution')).toHaveTextContent('Powered by GIPHY');
    expect(within(picker).queryByTestId('gif-more')).not.toBeInTheDocument();
  });

  it('searches what /giphy was given and sends the GIF picked', async () => {
    fetch.mockImplementation(() => answer(giphyPage(['cat1', 'cat2'])));
    const picker = open('happy cat');

    const [first] = await within(picker).findAllByTestId('gif-choice');
    expect(askedFor(0).searchParams.get('q')).toBe('happy cat');
    expect(screen.getByTestId('gif-search')).toHaveValue('happy cat');
    fireEvent.click(first!);

    expect(onPick).toHaveBeenCalledExactlyOnceWith(
      expect.objectContaining({ provider: 'giphy', id: 'cat1', title: 'Cat cat1' }),
    );
  });

  it('searches again once typing settles', async () => {
    fetch.mockImplementation(() => answer(giphyPage(['a'])));
    open();
    await screen.findByTestId('gif-choice');

    fireEvent.change(screen.getByTestId('gif-search'), { target: { value: 'dog' } });

    await waitFor(() => expect(fetch).toHaveBeenCalledTimes(2));
    expect(askedFor(1).searchParams.get('q')).toBe('dog');
  });

  it('says when nothing matches', async () => {
    fetch.mockImplementation(() => answer(giphyPage([])));
    const picker = open('zzzz');
    expect(await within(picker).findByTestId('gif-none')).toBeInTheDocument();
  });

  it('says when GIPHY cannot be reached', async () => {
    fetch.mockImplementation(() => answer({ message: 'Invalid key' }, 403));
    const picker = open();
    expect(await within(picker).findByTestId('gif-error')).toBeInTheDocument();
  });

  it('loads the next page on request', async () => {
    const ids = (from: number) => Array.from({ length: GIF_PAGE }, (_, i) => `g${from + i}`);
    fetch
      .mockImplementationOnce(() => answer(giphyPage(ids(0), 100, 0)))
      .mockImplementationOnce(() => answer(giphyPage(ids(GIF_PAGE), 100, GIF_PAGE)));
    const picker = open('cat');

    fireEvent.click(await within(picker).findByTestId('gif-more'));

    await waitFor(() => expect(within(picker).getAllByTestId('gif-choice')).toHaveLength(GIF_PAGE * 2));
    expect(askedFor(1).searchParams.get('offset')).toBe(String(GIF_PAGE));
  });

  it('moves from the search box into the grid with the arrow keys', async () => {
    fetch.mockImplementation(() => answer(giphyPage(['a', 'b', 'c'])));
    const picker = open();
    const choices = await within(picker).findAllByTestId('gif-choice');
    const search = screen.getByTestId('gif-search');

    fireEvent.keyDown(search, { key: 'ArrowDown' });
    expect(choices[0]).toHaveFocus();
    fireEvent.keyDown(choices[0]!, { key: 'ArrowRight' });
    expect(choices[1]).toHaveFocus();
    fireEvent.keyDown(choices[1]!, { key: 'ArrowDown' });
    expect(choices[2]).toHaveFocus();
    fireEvent.keyDown(choices[2]!, { key: 'ArrowUp' });
    expect(choices[0]).toHaveFocus();
    fireEvent.keyDown(choices[0]!, { key: 'ArrowUp' });
    expect(search).toHaveFocus();
  });
});
