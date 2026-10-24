import { useEffect, useRef, useState, type DragEvent, type KeyboardEvent } from 'react';
import { MessageSquare, Plus } from 'lucide-react';
import { CellContent, ChoiceChip } from './CellContent';
import { cellText, choiceOf, type Choice, type ListColumn, type ListItem } from './listModel';
import type { People } from './people';

const LANE_PAGE = 50;
const CARD_FIELDS = 3;

interface Props {
  label: string;
  group: ListColumn | null;
  columns: ListColumn[];
  items: ListItem[];
  canEdit: boolean;
  canAdd: boolean;
  hasGroupable: boolean;
  people: People;
  openItemId: string | null;
  onOpenItem: (itemId: string) => void;
  onSetGroup: (itemId: string, choiceId: string | null) => void;
  onAddItem: (choiceId: string | null) => void;
}

interface Lane {
  key: string;
  choice: Choice | null;
  items: ListItem[];
}

const laneKey = (choice: Choice | null) => choice?.id ?? 'none';

function lanesOf(group: ListColumn, items: ListItem[]): Lane[] {
  const choices = group.options.choices ?? [];
  const lanes: Lane[] = [...choices, null].map((choice) => ({ key: laneKey(choice), choice, items: [] }));
  const byKey = new Map(lanes.map((lane) => [lane.key, lane]));
  for (const item of items) {
    const choice = choiceOf(group, item.values[group.id]) ?? null;
    byKey.get(laneKey(choice))?.items.push(item);
  }
  return lanes;
}

export function ListBoard({
  label,
  group,
  columns,
  items,
  canEdit,
  canAdd,
  hasGroupable,
  people,
  openItemId,
  onOpenItem,
  onSetGroup,
  onAddItem,
}: Props) {
  const boardRef = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  const [over, setOver] = useState<string | null>(null);
  const [shown, setShown] = useState<Record<string, number>>({});
  const followRef = useRef<{ itemId: string; laneIndex: number } | null>(null);

  useEffect(() => {
    const follow = followRef.current;
    if (!follow) return;
    const card = boardRef.current?.querySelector<HTMLElement>(
      `[data-lane-index="${follow.laneIndex}"] [data-qa="list-card"][data-item-id="${follow.itemId}"]`,
    );
    if (!card) return;
    followRef.current = null;
    card.focus();
  });

  if (!group) {
    return (
      <div
        className="flex flex-1 items-center justify-center px-6 text-center text-sm text-muted"
        data-qa="list-board-ungrouped"
      >
        {hasGroupable
          ? 'Pick a single-select column to group this board by.'
          : 'Add a single-select column, like Status, to group this board by.'}
      </div>
    );
  }

  const lanes = lanesOf(group, items);
  const title = columns.find((column) => column.kind === 'text' && column.id !== group.id);
  const fields = columns.filter((column) => column.id !== group.id && column.id !== title?.id);

  const cardsIn = (laneIndex: number) => [
    ...(boardRef.current?.querySelectorAll<HTMLElement>(
      `[data-lane-index="${laneIndex}"] [data-qa="list-card"]`,
    ) ?? []),
  ];

  const onCardKeyDown = (laneIndex: number, cardIndex: number, item: ListItem) => (e: KeyboardEvent) => {
    const sideways = e.key === 'ArrowLeft' ? -1 : e.key === 'ArrowRight' ? 1 : 0;
    if (e.altKey && sideways !== 0) {
      e.preventDefault();
      const target = lanes[laneIndex + sideways];
      if (!canEdit || !target) return;
      followRef.current = { itemId: item.id, laneIndex: laneIndex + sideways };
      onSetGroup(item.id, target.choice?.id ?? null);
      return;
    }
    if (sideways !== 0) {
      e.preventDefault();
      for (let lane = laneIndex + sideways; lane >= 0 && lane < lanes.length; lane += sideways) {
        const cards = cardsIn(lane);
        if (cards.length > 0) {
          cards[Math.min(cardIndex, cards.length - 1)].focus();
          return;
        }
      }
      return;
    }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      cardsIn(laneIndex)[cardIndex + (e.key === 'ArrowDown' ? 1 : -1)]?.focus();
    }
  };

  const onLaneDragOver = (lane: Lane) => (e: DragEvent) => {
    if (!dragging) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    if (over !== lane.key) setOver(lane.key);
  };

  const onLaneDrop = (lane: Lane) => (e: DragEvent) => {
    e.preventDefault();
    const item = items.find((i) => i.id === dragging);
    if (item && laneKey(choiceOf(group, item.values[group.id]) ?? null) !== lane.key) {
      onSetGroup(item.id, lane.choice?.id ?? null);
    }
    setDragging(null);
    setOver(null);
  };

  return (
    <div
      ref={boardRef}
      role="region"
      aria-label={`${label}, grouped by ${group.name}`}
      data-qa="list-board"
      className="flex min-h-0 flex-1 gap-3 overflow-x-auto p-3"
    >
      {lanes.map((lane, laneIndex) => {
        const limit = shown[lane.key] ?? LANE_PAGE;
        const laneName = lane.choice?.label ?? `No ${group.name}`;
        return (
          <section
            key={lane.key}
            aria-label={`${laneName}, ${lane.items.length} ${lane.items.length === 1 ? 'item' : 'items'}`}
            data-qa="list-lane"
            data-lane={laneName}
            data-lane-index={laneIndex}
            onDragOver={onLaneDragOver(lane)}
            onDragLeave={(e) => {
              if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setOver(null);
            }}
            onDrop={onLaneDrop(lane)}
            className={`flex w-72 shrink-0 flex-col rounded-lg border bg-surface/40 transition ${
              over === lane.key ? 'border-accent/70 bg-accent/5' : 'border-line/50'
            }`}
          >
            <header className="flex items-center gap-2 px-3 py-2">
              {lane.choice ? (
                <ChoiceChip choice={lane.choice} />
              ) : (
                <span className="text-xs font-medium text-muted">{laneName}</span>
              )}
              <span className="text-xs tabular-nums text-subtle" data-qa="list-lane-count">
                {lane.items.length}
              </span>
            </header>
            <ul className="flex min-h-12 flex-1 flex-col gap-2 overflow-y-auto px-2 pb-2">
              {lane.items.slice(0, limit).map((item, cardIndex) => {
                const name = title ? cellText(title, item.values[title.id], people.nameOf) : '';
                const filled = fields
                  .filter((column) => {
                    const value = item.values[column.id];
                    return (
                      value !== undefined && value !== null && !(Array.isArray(value) && value.length === 0)
                    );
                  })
                  .slice(0, CARD_FIELDS);
                return (
                  <li key={item.id}>
                    <button
                      draggable={canEdit}
                      onDragStart={(e) => {
                        e.dataTransfer.effectAllowed = 'move';
                        e.dataTransfer.setData('text/plain', item.id);
                        setDragging(item.id);
                      }}
                      onDragEnd={() => {
                        setDragging(null);
                        setOver(null);
                      }}
                      onClick={() => onOpenItem(item.id)}
                      onKeyDown={onCardKeyDown(laneIndex, cardIndex, item)}
                      aria-current={openItemId === item.id || undefined}
                      aria-keyshortcuts={canEdit ? 'Alt+ArrowLeft Alt+ArrowRight' : undefined}
                      data-qa="list-card"
                      data-item-id={item.id}
                      className={`w-full rounded-md border bg-surface px-3 py-2 text-left shadow-sm transition cursor-pointer hover:border-line-strong focus:outline-none focus-visible:ring-2 focus-visible:ring-purple-500 ${
                        openItemId === item.id ? 'border-accent/70' : 'border-line/60'
                      } ${dragging === item.id ? 'opacity-50' : ''}`}
                    >
                      <span
                        className={`block truncate text-sm ${name ? 'text-fg' : 'text-subtle'}`}
                        data-qa="list-card-title"
                      >
                        {name || 'Untitled'}
                      </span>
                      {filled.length > 0 && (
                        <span className="mt-1.5 flex flex-col gap-1 text-xs text-fg-soft">
                          {filled.map((column) => (
                            <span key={column.id} className="flex min-w-0 items-center gap-1.5">
                              <span className="sr-only">{column.name}:</span>
                              {column.kind === 'link' ? (
                                <span className="truncate">
                                  {cellText(column, item.values[column.id], people.nameOf)}
                                </span>
                              ) : (
                                <CellContent column={column} value={item.values[column.id]} people={people} />
                              )}
                            </span>
                          ))}
                        </span>
                      )}
                      {item.comment_count > 0 && (
                        <span
                          className="mt-1.5 flex items-center gap-1 text-xs tabular-nums text-muted"
                          data-qa="list-card-comments"
                        >
                          <MessageSquare className="h-3 w-3" aria-hidden />
                          {item.comment_count}
                          <span className="sr-only">
                            {item.comment_count === 1 ? ' comment' : ' comments'}
                          </span>
                        </span>
                      )}
                    </button>
                  </li>
                );
              })}
            </ul>
            {lane.items.length > limit && (
              <button
                onClick={() => setShown((s) => ({ ...s, [lane.key]: limit + LANE_PAGE }))}
                data-qa="list-lane-more"
                className="mx-2 mb-2 rounded-md px-2 py-1 text-xs text-muted hover:bg-raised/50 hover:text-fg cursor-pointer"
              >
                Show {Math.min(LANE_PAGE, lane.items.length - limit)} more
              </button>
            )}
            {canEdit && (
              <button
                onClick={() => onAddItem(lane.choice?.id ?? null)}
                disabled={!canAdd}
                aria-label={`New item in ${laneName}`}
                data-qa="list-lane-add"
                className="mx-2 mb-2 inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-sm text-muted transition hover:bg-raised/50 hover:text-fg disabled:cursor-not-allowed disabled:opacity-50 cursor-pointer"
              >
                <Plus className="h-4 w-4" />
                New
              </button>
            )}
          </section>
        );
      })}
    </div>
  );
}
