import { lazy, Suspense, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Clapperboard } from 'lucide-react';
import type { RecordedClip } from '../lib/finish';
import { canRecordClips } from '../lib/format';

const ClipRecorderDialog = lazy(() => import('./ClipRecorderDialog'));

interface Props {
  onClip: (clip: RecordedClip) => void;
  className?: string;
}

export function ClipButton({ onClip, className = '' }: Props) {
  const { t } = useTranslation('messages');
  const [open, setOpen] = useState(false);
  if (!canRecordClips()) return null;
  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        aria-label={t('clips.record')}
        data-tooltip={t('clips.record')}
        data-qa="composer-clip"
        className={className}
      >
        <Clapperboard className="w-3.5 h-3.5" />
      </button>
      {open && (
        <Suspense fallback={null}>
          <ClipRecorderDialog onClip={onClip} onClose={() => setOpen(false)} />
        </Suspense>
      )}
    </>
  );
}
