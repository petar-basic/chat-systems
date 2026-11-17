import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Circle, Mic, Monitor, Square, Video } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { CLIP_COUNTDOWN_MS, CLIP_MAX_MS, clipClock, type ClipMode } from '../lib/format';
import type { RecordedClip } from '../lib/finish';
import { useClipRecorder } from '../hooks/useClipRecorder';
import { ClipPlayer } from './ClipPlayer';

interface Props {
  onClip: (clip: RecordedClip) => void;
  onClose: () => void;
}

function canShareScreen(): boolean {
  return typeof navigator !== 'undefined' && typeof navigator.mediaDevices?.getDisplayMedia === 'function';
}

function LivePreview({ stream }: { stream: MediaStream }) {
  const video = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    if (video.current) video.current.srcObject = stream;
  }, [stream]);
  return (
    <video
      ref={video}
      autoPlay
      muted
      playsInline
      data-qa="clip-live"
      className="aspect-video w-full rounded-lg border border-line/60 bg-black object-contain"
    />
  );
}

export default function ClipRecorderDialog({ onClip, onClose }: Props) {
  const { t } = useTranslation('messages');
  const { state, start, stop, again, take } = useClipRecorder();
  const [mode, setMode] = useState<ClipMode>(canShareScreen() ? 'screen' : 'camera');
  const [bubble, setBubble] = useState(false);

  const use = () => {
    const clip = take();
    if (clip) onClip(clip);
    onClose();
  };

  const modes = [
    {
      value: 'audio' as const,
      label: t('clips.modes.audio'),
      description: t('clips.modes.audioHint'),
      icon: <Mic className="h-4 w-4" />,
      dataQa: 'clip-mode-audio',
    },
    {
      value: 'camera' as const,
      label: t('clips.modes.camera'),
      description: t('clips.modes.cameraHint'),
      icon: <Video className="h-4 w-4" />,
      dataQa: 'clip-mode-camera',
    },
    ...(canShareScreen()
      ? [
          {
            value: 'screen' as const,
            label: t('clips.modes.screen'),
            description: t('clips.modes.screenHint'),
            icon: <Monitor className="h-4 w-4" />,
            dataQa: 'clip-mode-screen',
          },
        ]
      : []),
  ];

  let body: React.ReactNode = null;
  let actions: React.ReactNode = null;
  switch (state.phase) {
    case 'choosing':
    case 'starting':
      body = (
        <div className="flex flex-col gap-4 py-2">
          <RadioCards label={t('clips.what')} value={mode} options={modes} onChange={setMode} columns={1} />
          {mode === 'screen' && (
            <Checkbox
              label={t('clips.bubble')}
              description={t('clips.bubbleHint')}
              checked={bubble}
              onChange={setBubble}
              dataQa="clip-bubble"
            />
          )}
        </div>
      );
      actions = (
        <Button
          variant="primary"
          icon={<Circle className="h-3.5 w-3.5 fill-current" />}
          loading={state.phase === 'starting'}
          onClick={() => void start(mode, mode === 'screen' && bubble)}
          data-qa="clip-start"
        >
          {t('clips.start')}
        </Button>
      );
      break;
    case 'recording': {
      const left = CLIP_MAX_MS - state.elapsedMs;
      const ending = left <= CLIP_COUNTDOWN_MS;
      body = (
        <div className="flex flex-col gap-3 py-2">
          {state.preview ? (
            <LivePreview stream={state.preview} />
          ) : (
            <div className="flex h-24 items-center justify-center rounded-lg border border-line bg-raised/40 text-muted">
              <Mic className="h-6 w-6" />
            </div>
          )}
          <div className="flex items-center gap-2 text-sm" data-qa="clip-clock">
            <span className="h-2.5 w-2.5 animate-pulse rounded-full bg-red-500" />
            <span className="tabular-nums text-fg-soft">
              {t('clips.elapsed', { elapsed: clipClock(state.elapsedMs), limit: clipClock(CLIP_MAX_MS) })}
            </span>
            {ending && (
              <span
                role="timer"
                aria-live="polite"
                className="ml-auto font-medium text-warning"
                data-qa="clip-countdown"
              >
                {t('clips.left', { left: clipClock(left) })}
              </span>
            )}
          </div>
        </div>
      );
      actions = (
        <Button
          variant="danger"
          icon={<Square className="h-3.5 w-3.5 fill-current" />}
          onClick={() => void stop()}
          data-qa="clip-stop"
        >
          {t('clips.stop')}
        </Button>
      );
      break;
    }
    case 'processing':
      body = (
        <div className="flex items-center gap-3 py-8 text-sm text-muted">
          <Spinner label={t('clips.processing')} />
          {t('clips.processing')}
        </div>
      );
      break;
    case 'ready':
      body = (
        <div className="flex flex-col gap-2 py-2">
          <ClipPlayer
            url={state.clip.previewUrl}
            posterUrl={state.clip.posterUrl}
            clip={state.clip.clip}
            filename={state.clip.file.name}
          />
          <p className="text-xs text-muted">{t('clips.useHint')}</p>
        </div>
      );
      actions = (
        <>
          <Button onClick={again} data-qa="clip-again">
            {t('clips.again')}
          </Button>
          <Button variant="primary" onClick={use} data-qa="clip-use">
            {t('clips.use')}
          </Button>
        </>
      );
      break;
    case 'failed':
      body = (
        <p role="alert" className="py-4 text-sm text-danger" data-qa="clip-error">
          {state.error}
        </p>
      );
      actions = (
        <Button variant="primary" onClick={again} data-qa="clip-retry">
          {t('clips.tryAgain')}
        </Button>
      );
      break;
  }

  return (
    <Modal title={t('clips.title')} onClose={onClose} actions={actions} dataQa="clip-recorder">
      {body}
    </Modal>
  );
}
