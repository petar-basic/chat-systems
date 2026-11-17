import { useTranslation } from 'react-i18next';
import { Calendar, ChevronDown, MoreHorizontal } from 'lucide-react';
import type { components } from '@/api/schema';
import { CachedImage } from '@/shared/components/Image/CachedImage';
import { safeLink } from './layout';

type BlockElement = components['schemas']['BlockElement'];

const BUTTON_BASE =
  'inline-flex min-h-8 max-w-full items-center gap-1.5 rounded-md border px-3 py-1 text-sm font-semibold transition';

const BUTTON_STYLE = {
  primary: 'border-emerald-700 bg-emerald-700 text-white',
  danger: 'border-red-700 bg-red-700 text-white',
  default: 'border-line-strong bg-surface text-fg',
} as const;

interface Props {
  element: BlockElement;
}

export function BlockElementView({ element }: Props) {
  const { t } = useTranslation('messages');
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
