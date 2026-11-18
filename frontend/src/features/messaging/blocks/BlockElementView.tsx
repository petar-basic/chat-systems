import { useTranslation } from 'react-i18next';
import { Calendar, ChevronDown, MoreHorizontal } from 'lucide-react';
import type { components } from '@/api/schema';
import { CachedImage } from '@/shared/components/Image/CachedImage';
import { safeLink } from './layout';
import { BUTTON_BASE, BUTTON_STYLE } from './buttonStyles';
import { actionKey, useBlockActions } from './blockActions';
import { LiveElement } from './LiveElement';

type BlockElement = components['schemas']['BlockElement'];

interface Props {
  element: BlockElement;
  blockId?: string | null;
}

export function BlockElementView({ element, blockId }: Props) {
  const { t } = useTranslation('messages');
  const actions = useBlockActions();
  const inert = t('blocks.notInteractive');

  if (element.type === 'image') {
    return (
      <CachedImage
        src={element.image_url}
        alt={element.alt_text}
        referrerPolicy="no-referrer"
        data-qa="block-element-image"
        className="h-20 w-20 shrink-0 rounded-md border border-line/60 object-cover"
      />
    );
  }

  if (element.type === 'button') {
    const look = BUTTON_STYLE[element.style ?? 'default'];
    const href = safeLink(element.url);
    if (href) {
      return (
        <a
          href={href}
          target="_blank"
          rel="noopener noreferrer nofollow"
          data-qa="block-button"
          className={`${BUTTON_BASE} ${look} hover:brightness-110`}
        >
          <span className="truncate">{element.text.text}</span>
        </a>
      );
    }
  }

  const actionId = element.action_id;
  if (actions && blockId && actionId) {
    return (
      <LiveElement
        element={element}
        pending={actions.pending === actionKey(blockId, actionId)}
        act={(value) => actions.act(blockId, actionId, value)}
      />
    );
  }

  if (element.type === 'button') {
    const look = BUTTON_STYLE[element.style ?? 'default'];
    return (
      <button
        type="button"
        aria-disabled="true"
        data-tooltip={inert}
        data-qa="block-button"
        className={`${BUTTON_BASE} ${look} cursor-not-allowed opacity-60`}
      >
        <span className="truncate">{element.text.text}</span>
      </button>
    );
  }

  if (element.type === 'overflow') {
    return (
      <button
        type="button"
        aria-disabled="true"
        aria-label={t('blocks.more')}
        data-tooltip={inert}
        data-qa="block-overflow"
        className={`${BUTTON_BASE} ${BUTTON_STYLE.default} cursor-not-allowed px-2 opacity-60`}
      >
        <MoreHorizontal className="h-4 w-4" />
      </button>
    );
  }

  if (
    element.type === 'plain_text_input' ||
    element.type === 'checkboxes' ||
    element.type === 'radio_buttons'
  ) {
    return null;
  }

  const shown =
    element.type === 'static_select'
      ? (element.initial_option?.text.text ?? element.placeholder?.text ?? t('blocks.select'))
      : element.type === 'users_select'
        ? (element.placeholder?.text ?? t('blocks.selectPerson'))
        : element.type === 'conversations_select'
          ? (element.placeholder?.text ?? t('blocks.selectConversation'))
          : (element.initial_date ?? element.placeholder?.text ?? t('blocks.selectDate'));
  const Icon = element.type === 'datepicker' ? Calendar : ChevronDown;

  return (
    <button
      type="button"
      aria-disabled="true"
      data-tooltip={inert}
      data-qa="block-select"
      data-kind={element.type}
      className={`${BUTTON_BASE} ${BUTTON_STYLE.default} cursor-not-allowed justify-between font-normal opacity-60`}
    >
      <span className="truncate">{shown}</span>
      <Icon className="h-4 w-4 shrink-0 text-muted" />
    </button>
  );
}
