import { useRef, useState } from 'react';
import { Smile, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Select } from '@/shared/components/Form/Select';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { ReactionEmoji } from '@/shared/components/ReactionEmoji';
import { EmojiPicker } from '@/features/messaging';
import { formatDateTime } from '@/lib/datetime';
import type { StatusDraft } from './statusDraft';

const PRESETS = [
  { id: 'meeting', emoji: '📅', minutes: 60 },
  { id: 'lunch', emoji: '🍕', minutes: 60 },
  { id: 'sick', emoji: '🤒', minutes: 60 * 8 },
  { id: 'holiday', emoji: '🌴', minutes: null },
] as const;

const durations = (t: TFunction<'profile'>) => [
  { label: t('status.durations.never'), minutes: null },
  { label: t('status.durations.minutes', { count: 30 }), minutes: 30 },
  { label: t('status.durations.hours', { count: 1 }), minutes: 60 },
  { label: t('status.durations.hours', { count: 4 }), minutes: 240 },
  { label: t('status.durations.today'), minutes: 60 * 12 },
];

interface Props {
  value: StatusDraft;
  onChange: (next: StatusDraft) => void;
  clearsAt?: string | null;
}

export function StatusFields({ value, onChange, clearsAt }: Props) {
  const { t } = useTranslation('profile');
  const hasStatus = !!(value.emoji || value.text);
  const emojiButton = useRef<HTMLButtonElement>(null);
  const [picking, setPicking] = useState(false);

  return (
    <fieldset data-qa="status-editor">
      <legend className="mb-1.5 text-sm font-medium text-fg-dim">{t('status.label')}</legend>
      <div className="flex items-center gap-2">
        <button
          ref={emojiButton}
          type="button"
          onClick={() => setPicking((open) => !open)}
          aria-label={value.emoji ? t('status.emoji', { emoji: value.emoji }) : t('status.chooseEmoji')}
          data-tooltip={value.emoji ? t('status.emoji', { emoji: value.emoji }) : t('status.chooseEmoji')}
          aria-expanded={picking}
          data-qa="status-emoji"
          className="flex h-9 w-11 shrink-0 cursor-pointer items-center justify-center rounded-lg border border-line bg-raised/50 text-lg leading-none transition hover:bg-raised"
        >
          {value.emoji ? <ReactionEmoji emoji={value.emoji} /> : <Smile className="h-4 w-4 text-muted" />}
        </button>
        {picking && (
          <EmojiPicker
            anchorRef={emojiButton}
            onSelect={(emoji) => onChange({ ...value, emoji })}
            onClose={() => setPicking(false)}
          />
        )}
        <TextInput
          label={t('status.label')}
          labelHidden
          value={value.text}
          onChange={(e) => onChange({ ...value, text: e.target.value })}
          placeholder={t('status.placeholder')}
          maxLength={100}
          dataQa="status-text"
          className="flex-1"
          trailing={
            hasStatus && (
              <IconButton
                label={t('status.clear')}
                icon={<X className="h-3.5 w-3.5" />}
                onClick={() => onChange({ emoji: '', text: '', minutes: null })}
                data-qa="status-clear"
              />
            )
          }
        />
      </div>

      <div className="mt-2 flex flex-wrap gap-1.5">
        {PRESETS.map((preset) => {
          const text = t(`status.presets.${preset.id}`);
          return (
            <button
              key={preset.id}
              type="button"
              onClick={() => onChange({ emoji: preset.emoji, text, minutes: preset.minutes })}
              data-qa="status-preset"
              className="cursor-pointer rounded-md bg-raised/40 px-2 py-1 text-xs text-fg-dim transition hover:bg-raised"
            >
              {preset.emoji} {text}
            </button>
          );
        })}
      </div>

      {hasStatus && !clearsAt && (
        <Select
          label={t('status.clearAfter')}
          compact
          value={value.minutes === null ? '' : String(value.minutes)}
          onChange={(e) =>
            onChange({ ...value, minutes: e.target.value === '' ? null : Number(e.target.value) })
          }
          dataQa="status-duration"
          className="mt-3 w-44"
        >
          {durations(t).map((option) => (
            <option key={option.label} value={option.minutes === null ? '' : String(option.minutes)}>
              {option.label}
            </option>
          ))}
        </Select>
      )}
      {clearsAt && (
        <p className="mt-1.5 text-xs text-muted" data-qa="status-expiry">
          {t('status.clearsAt', { time: formatDateTime(clearsAt) })}
        </p>
      )}
    </fieldset>
  );
}
