import type { KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { History, X } from 'lucide-react';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { dateLabel, hasLabel, kindLabel, memberName, placeName, type FilterContext } from './searchFilters';
import { filtersOf, type RecentSearch } from './useRecentSearches';

export const RECENT_ITEM = '[data-qa="recent-search"]';

interface Props {
  searches: RecentSearch[];
  context: FilterContext;
  onPick: (search: RecentSearch) => void;
  onForget: (search: RecentSearch) => void;
  onClear: () => void;
  onLeave: () => void;
}

export function RecentSearches({ searches, context, onPick, onForget, onClear, onLeave }: Props) {
  const { t } = useTranslation('search');

  const chipsOf = (search: RecentSearch) => {
    const filters = filtersOf(search);
    const date = dateLabel(filters);
    return [
      search.tab === 'files' && t('panel.tabs.files'),
      filters.from && `${t('filters.from')} ${memberName(context, filters.from)}`,
      filters.in && `${t('filters.in')} ${placeName(context, filters.in)}`,
      filters.has && hasLabel(filters.has),
      filters.kind && kindLabel(filters.kind),
      date,
    ].filter((chip): chip is string => !!chip);
  };

  const move = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    const items = Array.from(
      event.currentTarget
        .closest('[data-qa="recent-searches"]')
        ?.querySelectorAll<HTMLButtonElement>(RECENT_ITEM) ?? [],
    );
    const at = items.indexOf(event.currentTarget);
    const next = event.key === 'ArrowDown' ? at + 1 : at - 1;
    if (next < 0) onLeave();
    else items[Math.min(next, items.length - 1)]?.focus();
  };

  return (
    <section className="px-1 py-1" data-qa="recent-searches" aria-labelledby="recent-searches-title">
      <h3 id="recent-searches-title" className="px-2 pb-1 text-xs font-semibold text-muted">
        {t('recent.title')}
      </h3>
      <ul className="space-y-0.5">
        {searches.map((search) => {
          const chips = chipsOf(search);
          const named = search.query || chips.join(t('listSeparator', { ns: 'common' }));
          return (
            <li key={search.id} className="flex items-center gap-1 rounded-lg hover:bg-raised/30">
              <button
                type="button"
                data-qa="recent-search"
                onClick={() => onPick(search)}
                onKeyDown={move}
                className="flex min-w-0 flex-1 items-center gap-2 rounded-lg px-2 py-1.5 text-left focus-visible:bg-raised/40 cursor-pointer"
              >
                <History className="h-3.5 w-3.5 shrink-0 text-muted" />
                <span className="min-w-0 flex-1">
                  {search.query && (
                    <span className="block truncate text-sm text-fg-soft" data-qa="recent-search-query">
                      {search.query}
                    </span>
                  )}
                  {chips.length > 0 && (
                    <span className="mt-0.5 flex flex-wrap gap-1">
                      {chips.map((chip) => (
                        <span
                          key={chip}
                          data-qa="recent-search-chip"
                          className="max-w-full truncate rounded-full border border-purple-500/40 bg-purple-600/10 px-1.5 text-2xs text-fg-dim"
                        >
                          {chip}
                        </span>
                      ))}
                    </span>
                  )}
                </span>
              </button>
              <IconButton
                label={t('recent.remove', { search: named })}
                icon={<X className="h-3.5 w-3.5" />}
                onClick={() => onForget(search)}
                data-qa="recent-search-remove"
                className="mr-1"
              />
            </li>
          );
        })}
      </ul>
      <div className="px-2 pt-1">
        <button
          type="button"
          onClick={onClear}
          data-qa="recent-searches-clear"
          className="text-xs text-accent hover:text-accent-soft cursor-pointer"
        >
          {t('recent.clear')}
        </button>
      </div>
    </section>
  );
}
