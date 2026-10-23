import { useRef, useState } from 'react';
import { AlarmClock } from 'lucide-react';
import { AnchoredPopover } from '@/shared/components/Popover/AnchoredPopover';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { REMIND_PRESETS, formatScheduleHint, toLocalInputValue } from '@/features/messaging/schedulePresets';
import { TextInput } from '@/shared/components/Form/TextInput';

interface Props {
  label?: string;
  onPick: (at: Date) => void;
  onClear?: () => void;
  dataQa?: string;
}

const ITEM =
  'w-full px-3 py-1.5 flex items-center justify-between gap-3 text-left text-sm text-fg-dim hover:bg-raised';

export function RemindMenu({ label = 'Remind me', onPick, onClear, dataQa }: Props) {
  const anchor = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [custom, setCustom] = useState('');
  const [earliest, setEarliest] = useState('');
  const pick = (at: Date) => {
    onPick(at);
    setOpen(false);
  };

  return (
    <>
      <IconButton
        ref={anchor}
        label={label}
        icon={<AlarmClock className="h-4 w-4" />}
        onClick={() => {
          setEarliest(toLocalInputValue(new Date(Date.now() + 60_000)));
          setOpen((value) => !value);
        }}
        data-qa={dataQa}
      />
      {open && (
        <AnchoredPopover
          anchorRef={anchor}
          onClose={() => setOpen(false)}
          dataQa="remind-menu"
          className="w-60 bg-surface border border-line rounded-lg shadow-xl py-1"
        >
          {REMIND_PRESETS.map((preset) => (
            <button key={preset.label} type="button" className={ITEM} onClick={() => pick(preset.at())}>
              <span>{preset.label}</span>
              <span className="text-[11px] text-subtle">{formatScheduleHint(preset.at())}</span>
            </button>
          ))}
          <div className="mt-1 border-t border-line/70 px-3 py-2">
            <TextInput
              label="Custom reminder time"
              labelHidden
              compact
              type="datetime-local"
              value={custom}
              min={earliest}
              onChange={(e) => setCustom(e.target.value)}
            />
            <button
              type="button"
              disabled={!custom || custom < earliest}
              onClick={() => pick(new Date(custom))}
              className="mt-1.5 w-full text-xs text-accent hover:underline disabled:text-faint disabled:no-underline"
            >
              Set custom time
            </button>
          </div>
          {onClear && (
            <button
              type="button"
              className={`${ITEM} border-t border-line/70 text-danger`}
              onClick={() => {
                onClear();
                setOpen(false);
              }}
            >
              Clear reminder
            </button>
          )}
        </AnchoredPopover>
      )}
    </>
  );
}
