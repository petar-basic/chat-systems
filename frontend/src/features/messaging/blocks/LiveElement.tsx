import { useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { MoreHorizontal } from 'lucide-react';
import type { components } from '@/api/schema';
import { Select } from '@/shared/components/Form/Select';
import { Combobox } from '@/shared/components/Form/Combobox';
import { TextInput } from '@/shared/components/Form/TextInput';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import { useWorkspaceStore } from '@/stores/workspace';
import { BUTTON_BASE, BUTTON_STYLE } from './buttonStyles';
import { useChannelChoices, usePersonChoices } from './choices';
import { safeLink } from './layout';

type BlockElement = components['schemas']['BlockElement'];
type BlockOption = components['schemas']['BlockOption'];

interface Props {
  element: Exclude<BlockElement, { type: 'image' }>;
  pending: boolean;
  act: (value?: string) => void;
}

export function LiveElement({ element, pending, act }: Props) {
  const { t } = useTranslation('messages');

  switch (element.type) {
    case 'button':
      return (
        <button
          type="button"
          onClick={() => act()}
          disabled={pending}
          aria-busy={pending || undefined}
          data-qa="block-button"
          className={`${BUTTON_BASE} ${BUTTON_STYLE[element.style ?? 'default']} cursor-pointer hover:brightness-110 disabled:cursor-wait disabled:opacity-60`}
        >
          <span className="truncate">{element.text.text}</span>
        </button>
      );
    case 'overflow':
      return <OverflowMenu options={element.options ?? []} pending={pending} act={act} />;
    case 'static_select':
      return (
        <Choice
          initial={element.initial_option?.value ?? ''}
          act={act}
          render={(value, choose) => (
            <Select
              label={element.placeholder?.text ?? t('blocks.select')}
              labelHidden
              compact
              value={value}
              disabled={pending}
              onChange={(e) => choose(e.target.value)}
              dataQa="block-select"
              className="min-w-40"
            >
              <option value="" disabled>
                {element.placeholder?.text ?? t('blocks.select')}
              </option>
              {(element.options ?? []).map((option) => (
                <option key={option.value} value={option.value}>
                  {option.text.text}
                </option>
              ))}
            </Select>
          )}
        />
      );
    case 'users_select':
      return (
        <PersonChoice
          label={element.placeholder?.text ?? t('blocks.selectPerson')}
          initial={element.initial_user ?? ''}
          pending={pending}
          act={act}
        />
      );
    case 'conversations_select':
      return (
        <ChannelChoice
          label={element.placeholder?.text ?? t('blocks.selectConversation')}
          initial={element.initial_conversation ?? ''}
          pending={pending}
          act={act}
        />
      );
    case 'datepicker':
      return (
        <Choice
          initial={element.initial_date ?? ''}
          act={act}
          render={(value, choose) => (
            <TextInput
              type="date"
              label={element.placeholder?.text ?? t('blocks.selectDate')}
              labelHidden
              compact
              value={value}
              disabled={pending}
              onChange={(e) => choose(e.target.value)}
              dataQa="block-datepicker"
            />
          )}
        />
      );
    default:
      return null;
  }
}

function Choice({
  initial,
  act,
  render,
}: {
  initial: string;
  act: (value?: string) => void;
  render: (value: string, choose: (value: string) => void) => ReactNode;
}) {
  const [value, setValue] = useState(initial);
  const choose = (next: string) => {
    setValue(next);
    if (next) act(next);
  };
  return <>{render(value, choose)}</>;
}

function PersonChoice({
  label,
  initial,
  pending,
  act,
}: {
  label: string;
  initial: string;
  pending: boolean;
  act: (value?: string) => void;
}) {
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const options = usePersonChoices(workspace?.id ?? null, workspace?.instanceUrl);
  return (
    <Choice
      initial={initial}
      act={act}
      render={(value, choose) => (
        <Combobox
          label={label}
          labelHidden
          placeholder={label}
          value={value}
          options={options}
          disabled={pending}
          onChange={choose}
          dataQa="block-users-select"
          className="w-56"
        />
      )}
    />
  );
}

function ChannelChoice({
  label,
  initial,
  pending,
  act,
}: {
  label: string;
  initial: string;
  pending: boolean;
  act: (value?: string) => void;
}) {
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const options = useChannelChoices(workspace?.id ?? null, workspace?.instanceUrl);
  return (
    <Choice
      initial={initial}
      act={act}
      render={(value, choose) => (
        <Combobox
          label={label}
          labelHidden
          placeholder={label}
          value={value}
          options={options}
          disabled={pending}
          onChange={choose}
          dataQa="block-conversations-select"
          className="w-56"
        />
      )}
    />
  );
}

function OverflowMenu({
  options,
  pending,
  act,
}: {
  options: BlockOption[];
  pending: boolean;
  act: (value?: string) => void;
}) {
  const { t } = useTranslation('messages');
  const anchor = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const pick = (option: BlockOption) => {
    setOpen(false);
    const href = safeLink(option.url);
    if (href) window.open(href, '_blank', 'noopener,noreferrer');
    act(option.value);
  };
  return (
    <>
      <button
        ref={anchor}
        type="button"
        aria-label={t('blocks.more')}
        aria-haspopup="menu"
        aria-expanded={open}
        data-tooltip={t('blocks.more')}
        disabled={pending}
        onClick={() => setOpen((was) => !was)}
        data-qa="block-overflow"
        className={`${BUTTON_BASE} ${BUTTON_STYLE.default} cursor-pointer px-2 hover:brightness-110 disabled:cursor-wait disabled:opacity-60`}
      >
        <MoreHorizontal className="h-4 w-4" />
      </button>
      {open && (
        <AnchoredPopover
          anchorRef={anchor}
          onClose={() => setOpen(false)}
          align="start"
          dataQa="block-overflow-menu"
          className="w-52 rounded-lg border border-line bg-surface p-1 shadow-xl"
        >
          <div role="menu" aria-label={t('blocks.more')}>
            {options.map((option, index) => (
              <button
                key={option.value ?? index}
                type="button"
                role="menuitem"
                data-autofocus={index === 0 || undefined}
                onClick={() => pick(option)}
                className="flex w-full cursor-pointer items-center rounded px-3 py-2 text-left text-sm text-fg-dim hover:bg-raised focus:bg-raised focus:outline-none"
              >
                {option.text.text}
              </button>
            ))}
          </div>
        </AnchoredPopover>
      )}
    </>
  );
}
