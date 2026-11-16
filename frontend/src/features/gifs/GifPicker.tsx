import { useState, type KeyboardEvent, type RefObject } from 'react';
import { useTranslation } from 'react-i18next';
import { Search } from 'lucide-react';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Button } from '@/shared/components/Button/Button';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { useDebouncedValue } from '@/shared/hooks/useDebouncedValue';
import { useGifSearch } from './useGifs';
import { useReducedMotion } from './useReducedMotion';
import type { Gif, GifSettings } from './types';

const TYPING_MS = 300;
const COLUMNS = 2;
export const GIF_CHOICE = '[data-qa="gif-choice"]';

interface Props {
  anchorRef: RefObject<HTMLElement | null>;
  settings: GifSettings;
  initialQuery?: string;
  onPick: (gif: Gif) => void;
  onClose: () => void;
}

export function GifPicker({ anchorRef, settings, initialQuery = '', onPick, onClose }: Props) {
  const { t } = useTranslation('messages');
  const [query, setQuery] = useState(initialQuery);
  const searched = useDebouncedValue(query, TYPING_MS);
  const reduced = useReducedMotion();
  const results = useGifSearch(settings, searched);
  const choices = results.data?.pages.flatMap((page) => page.choices) ?? [];

  const move = (event: KeyboardEvent<HTMLButtonElement>) => {
    const step = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: COLUMNS, ArrowUp: -COLUMNS }[event.key];
    if (!step) return;
    event.preventDefault();
    const all = Array.from(
      event.currentTarget
        .closest('[data-qa="gif-picker"]')
        ?.querySelectorAll<HTMLButtonElement>(GIF_CHOICE) ?? [],
    );
    const next = all.indexOf(event.currentTarget) + step;
    if (next < 0) event.currentTarget.closest('[data-qa="gif-picker"]')?.querySelector('input')?.focus();
    else all[Math.min(next, all.length - 1)]?.focus();
  };

  return (
    <AnchoredPopover anchorRef={anchorRef} onClose={onClose} dataQa="gif-picker">
      <div className="w-80 max-w-[calc(100vw-1rem)] rounded-xl border border-line bg-surface shadow-xl">
        <div className="p-2">
          <TextInput
            label={t('gifs.search')}
            labelHidden
            compact
            data-autofocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'ArrowDown') {
                e.preventDefault();
                e.currentTarget
                  .closest('[data-qa="gif-picker"]')
                  ?.querySelector<HTMLButtonElement>(GIF_CHOICE)
                  ?.focus();
              }
            }}
            placeholder={t('gifs.placeholder')}
            leading={<Search className="w-4 h-4" />}
            dataQa="gif-search"
          />
        </div>
        <div className="max-h-80 overflow-y-auto px-2" aria-live="polite" aria-busy={results.isFetching}>
          {results.isPending ? (
            <div className="flex justify-center py-8">
              <Spinner label={t('gifs.loading')} />
            </div>
          ) : results.isError ? (
            <p className="py-8 text-center text-sm text-danger" role="alert" data-qa="gif-error">
              {t('gifs.failed')}
            </p>
          ) : choices.length === 0 ? (
            <p className="py-8 text-center text-sm text-muted" data-qa="gif-none">
              {t('gifs.none')}
            </p>
          ) : (
            <ul
              className="grid grid-cols-2 gap-1.5"
              aria-label={searched.trim() ? t('gifs.results') : t('gifs.trending')}
            >
              {choices.map((choice) => (
                <li key={choice.gif.id}>
                  <button
                    type="button"
                    onClick={() => onPick(choice.gif)}
                    onKeyDown={move}
                    aria-label={choice.gif.title || t('gifs.untitled')}
                    data-tooltip={choice.gif.title || t('gifs.untitled')}
                    data-qa="gif-choice"
                    className="block w-full overflow-hidden rounded-md bg-raised/40 cursor-pointer focus-visible:outline-2 focus-visible:outline-accent"
                    style={{ aspectRatio: `${choice.width} / ${choice.height}` }}
                  >
                    <img
                      src={reduced ? choice.still : choice.preview}
                      alt=""
                      loading="lazy"
                      className="h-full w-full object-cover"
                    />
                  </button>
                </li>
              ))}
            </ul>
          )}
          {results.hasNextPage && (
            <div className="flex justify-center py-2">
              <Button
                size="sm"
                variant="ghost"
                loading={results.isFetchingNextPage}
                onClick={() => void results.fetchNextPage()}
                data-qa="gif-more"
              >
                {t('gifs.more')}
              </Button>
            </div>
          )}
        </div>
        <p
          className="px-3 py-2 text-right text-2xs font-semibold tracking-wide text-muted"
          data-qa="gif-attribution"
        >
          {t('gifs.poweredBy')}
        </p>
      </div>
    </AnchoredPopover>
  );
}
