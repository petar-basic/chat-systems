import { useRef, useState } from 'react';
import { Smile, X } from 'lucide-react';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Select } from '@/shared/components/Form/Select';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { ReactionEmoji } from '@/shared/components/ReactionEmoji';
import { EmojiPicker } from '@/features/messaging';
import { formatDateTime } from '@/lib/datetime';
import type { StatusDraft } from './statusDraft';

const PRESETS: StatusDraft[] = [
  { emoji: '📅', text: 'In a meeting', minutes: 60 },
  { emoji: '🍕', text: 'At lunch', minutes: 60 },
  { emoji: '🤒', text: 'Out sick', minutes: 60 * 8 },
  { emoji: '🌴', text: 'On holiday', minutes: null },
];

const DURATIONS = [
  { label: "Don't clear", minutes: null },
  { label: '30 minutes', minutes: 30 },
  { label: '1 hour', minutes: 60 },
  { label: '4 hours', minutes: 240 },
  { label: 'Today', minutes: 60 * 12 },
];

interface Props {
  value: StatusDraft;
  onChange: (next: StatusDraft) => void;
  clearsAt?: string | null;
}

export function StatusFields({ value, onChange, clearsAt }: Props) {
  const hasStatus = !!(value.emoji || value.text);
  const emojiButton = useRef<HTMLButtonElement>(null);
  const [picking, setPicking] = useState(false);

  return (
    <fieldset data-qa="status-editor">
      <legend className="mb-1.5 text-sm font-medium text-fg-dim">Status</legend>
      <div className="flex items-center gap-2">
        <button
          ref={emojiButton}
          type="button"
          onClick={() => setPicking((open) => !open)}
          aria-label={value.emoji ? `Status emoji ${value.emoji}` : 'Choose a status emoji'}
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
          label="Status"
          labelHidden
          value={value.text}
          onChange={(e) => onChange({ ...value, text: e.target.value })}
          placeholder="What's happening?"
          maxLength={100}
          dataQa="status-text"
          className="flex-1"
          trailing={
            hasStatus && (
              <IconButton
                label="Clear status"
                icon={<X className="h-3.5 w-3.5" />}
                onClick={() => onChange({ emoji: '', text: '', minutes: null })}
                data-qa="status-clear"
              />
            )
          }
        />
      </div>

      <div className="mt-2 flex flex-wrap gap-1.5">
        {PRESETS.map((preset) => (
          <button
            key={preset.text}
            type="button"
            onClick={() => onChange(preset)}
            data-qa="status-preset"
            className="cursor-pointer rounded-md bg-raised/40 px-2 py-1 text-xs text-fg-dim transition hover:bg-raised"
          >
            {preset.emoji} {preset.text}
          </button>
        ))}
      </div>

      {hasStatus && !clearsAt && (
        <Select
          label="Clear after"
          compact
          value={value.minutes === null ? '' : String(value.minutes)}
          onChange={(e) =>
            onChange({ ...value, minutes: e.target.value === '' ? null : Number(e.target.value) })
          }
          dataQa="status-duration"
          className="mt-3 w-44"
        >
          {DURATIONS.map((option) => (
            <option key={option.label} value={option.minutes === null ? '' : String(option.minutes)}>
              {option.label}
            </option>
          ))}
        </Select>
      )}
      {clearsAt && (
        <p className="mt-1.5 text-xs text-muted" data-qa="status-expiry">
          Clears at {formatDateTime(clearsAt)}
        </p>
      )}
    </fieldset>
  );
}
