import { useRef, useState, type ComponentType } from 'react';
import { Plus } from 'lucide-react';
import { ActionSheet } from '@/shared/components/ActionSheet/ActionSheet';
import EmojiPicker from './EmojiPicker';

export interface SheetAction {
  key: string;
  label: string;
  Icon: ComponentType<{ className?: string }>;
  onSelect: () => void;
  destructive?: boolean;
}

interface Props {
  quickReactions: string[];
  onReact: (emoji: string) => void;
  actions: SheetAction[];
  onClose: () => void;
  dataQa?: string;
}

export default function MessageActionSheet({
  quickReactions,
  onReact,
  actions,
  onClose,
  dataQa = 'message-action-sheet',
}: Props) {
  const [showPicker, setShowPicker] = useState(false);
  const pickerAnchorRef = useRef<HTMLButtonElement>(null);

  const react = (emoji: string) => {
    onReact(emoji);
    onClose();
  };

  return (
    <ActionSheet onClose={onClose} dataQa={dataQa} title="Message actions">
      <div className="flex items-center gap-2 px-4 py-3 border-b border-line">
        {quickReactions.map((emoji) => (
          <button
            key={emoji}
            type="button"
            onClick={() => react(emoji)}
            aria-label={`React with ${emoji}`}
            data-qa="sheet-quick-reaction"
            className="w-11 h-11 rounded-full bg-raised/60 hover:bg-raised text-xl flex items-center justify-center transition"
          >
            {emoji}
          </button>
        ))}
        <button
          ref={pickerAnchorRef}
          type="button"
          onClick={() => setShowPicker((open) => !open)}
          aria-label="More reactions"
          aria-expanded={showPicker}
          data-qa="sheet-more-reactions"
          className="w-11 h-11 rounded-full bg-raised/60 hover:bg-raised text-muted flex items-center justify-center transition"
        >
          <Plus className="w-5 h-5" />
        </button>
        {showPicker && (
          <EmojiPicker
            anchorRef={pickerAnchorRef}
            onSelect={(emoji) => {
              setShowPicker(false);
              react(emoji);
            }}
            onClose={() => setShowPicker(false)}
          />
        )}
      </div>

      <div className="py-1">
        {actions.map(({ key, label, Icon, onSelect, destructive }) => (
          <button
            key={key}
            type="button"
            data-qa={`sheet-action-${key}`}
            onClick={() => {
              onSelect();
              onClose();
            }}
            className={`w-full px-4 py-3.5 flex items-center gap-3 text-left text-sm transition active:bg-raised ${
              destructive ? 'text-danger' : 'text-fg-dim'
            }`}
          >
            <Icon className="w-4 h-4 shrink-0" />
            {label}
          </button>
        ))}
      </div>
    </ActionSheet>
  );
}
