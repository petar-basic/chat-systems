import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { createPortal } from 'react-dom';
import { ChevronDown } from 'lucide-react';
import { autoUpdate, flip, offset, size, useFloating } from '@floating-ui/react-dom';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';
import { Field, type FieldProps } from './Field';
import { fieldStyles } from './fieldStyles';

export interface ComboboxOption {
  value: string;
  label: string;
  detail?: string;
}

export interface ComboboxProps extends FieldProps {
  value: string;
  options: ComboboxOption[];
  onChange: (value: string) => void;
  filter?: (option: ComboboxOption, query: string) => boolean;
  placeholder?: string;
  emptyText?: string;
  disabled?: boolean;
  dataQa?: string;
}

const MAX_HEIGHT = 288;

function matchesText(option: ComboboxOption, query: string) {
  return `${option.label} ${option.detail ?? ''}`.toLowerCase().includes(query.trim().toLowerCase());
}

export function Combobox({
  label,
  labelHidden,
  optional,
  hint,
  error,
  className,
  value,
  options,
  onChange,
  filter = matchesText,
  placeholder,
  emptyText = 'Nothing matches',
  disabled = false,
  dataQa,
}: ComboboxProps) {
  const listId = useId();
  const list = useRef<HTMLUListElement | null>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);

  const selected = options.find((option) => option.value === value);
  const shown = useMemo(
    () => (query.trim() ? options.filter((option) => filter(option, query)) : options),
    [options, query, filter],
  );

  const { refs, floatingStyles, isPositioned } = useFloating({
    open,
    placement: 'bottom-start',
    strategy: 'fixed',
    middleware: [
      offset(4),
      flip({ padding: 8 }),
      size({
        padding: 8,
        apply({ rects, availableHeight, elements }) {
          Object.assign(elements.floating.style, {
            width: `${rects.reference.width}px`,
            maxHeight: `${Math.min(MAX_HEIGHT, availableHeight)}px`,
          });
        },
      }),
    ],
    whileElementsMounted: autoUpdate,
  });

  const close = () => {
    setOpen(false);
    setQuery('');
  };
  useEscapeToClose(close, open);

  useEffect(() => {
    const node = list.current;
    const item = node?.querySelector<HTMLElement>(`[data-index="${active}"]`);
    if (!node || !item || !isPositioned) return;
    if (item.offsetTop < node.scrollTop) node.scrollTop = item.offsetTop;
    else if (item.offsetTop + item.offsetHeight > node.scrollTop + node.clientHeight)
      node.scrollTop = item.offsetTop + item.offsetHeight - node.clientHeight;
  }, [active, isPositioned]);

  useEffect(() => {
    const node = list.current;
    const item = node?.querySelector<HTMLElement>('[aria-selected="true"]');
    if (!node || !item || !isPositioned) return;
    node.scrollTop = item.offsetTop - (node.clientHeight - item.offsetHeight) / 2;
  }, [isPositioned]);

  const show = () => {
    if (disabled || open) return;
    setActive(
      Math.max(
        0,
        options.findIndex((option) => option.value === value),
      ),
    );
    setOpen(true);
  };

  const choose = (option: ComboboxOption) => {
    if (option.value !== value) onChange(option.value);
    close();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (!open) return show();
      if (shown.length === 0) return;
      const step = e.key === 'ArrowDown' ? 1 : -1;
      setActive((at) => (at + step + shown.length) % shown.length);
    } else if (e.key === 'Enter' && open) {
      e.preventDefault();
      if (shown[active]) choose(shown[active]);
    } else if (e.key === 'Tab' && open) {
      close();
    }
  };

  return (
    <Field
      label={label}
      labelHidden={labelHidden}
      optional={optional}
      hint={hint}
      error={error}
      className={className}
    >
      {(control) => (
        <div className="relative" ref={refs.setReference}>
          <input
            {...control}
            type="text"
            role="combobox"
            aria-expanded={open}
            aria-controls={listId}
            aria-autocomplete="list"
            aria-activedescendant={open && shown[active] ? `${listId}-${active}` : undefined}
            autoComplete="off"
            spellCheck={false}
            value={open ? query : (selected?.label ?? value)}
            placeholder={open ? (selected?.label ?? placeholder) : placeholder}
            disabled={disabled}
            onClick={show}
            onChange={(e) => {
              setQuery(e.target.value);
              setActive(0);
              setOpen(true);
            }}
            onKeyDown={onKeyDown}
            onBlur={close}
            data-qa={dataQa}
            className={`h-9 cursor-pointer ${fieldStyles({ invalid: !!error, trailing: true })}`}
          />
          <ChevronDown className="pointer-events-none absolute top-1/2 right-2.5 h-4 w-4 -translate-y-1/2 text-muted" />
          {open &&
            createPortal(
              <ul
                ref={(node) => {
                  list.current = node;
                  refs.setFloating(node);
                }}
                id={listId}
                role="listbox"
                aria-label={label}
                style={floatingStyles}
                onMouseDown={(e) => e.preventDefault()}
                data-qa={dataQa ? `${dataQa}-options` : undefined}
                className="z-70 overflow-y-auto rounded-lg border border-line bg-surface py-1 shadow-xl"
              >
                {shown.length === 0 ? (
                  <li role="presentation" className="px-3 py-2 text-sm text-muted">
                    {emptyText}
                  </li>
                ) : (
                  shown.map((option, index) => (
                    <li
                      key={option.value}
                      id={`${listId}-${index}`}
                      data-index={index}
                      role="option"
                      aria-selected={option.value === value}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        choose(option);
                      }}
                      onMouseEnter={() => setActive(index)}
                      className={`flex cursor-pointer items-baseline justify-between gap-3 px-3 py-1.5 text-sm ${
                        index === active ? 'bg-raised' : ''
                      } ${option.value === value ? 'font-medium text-fg' : 'text-fg-soft'}`}
                    >
                      <span className="truncate">{option.label}</span>
                      {option.detail && (
                        <span className="shrink-0 text-xs font-normal text-muted">{option.detail}</span>
                      )}
                    </li>
                  ))
                )}
              </ul>,
              document.body,
            )}
        </div>
      )}
    </Field>
  );
}
