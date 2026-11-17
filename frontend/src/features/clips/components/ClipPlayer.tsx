import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Download, Expand, Pause, Play } from 'lucide-react';
import type { components } from '@/api/schema';
import { clipClock } from '../lib/format';

type Clip = components['schemas']['Clip'];

const SPEEDS = [1, 1.5, 2] as const;
const SEEK_STEP_S = 5;
const VIDEO_MAX_WIDTH = 480;
const VIDEO_MAX_HEIGHT = 320;

interface Props {
  url: string;
  clip: Clip;
  filename: string;
  posterUrl?: string | null;
  width?: number | null;
  height?: number | null;
  onExpand?: () => void;
}

export function ClipPlayer({ url, clip, filename, posterUrl, width, height, onExpand }: Props) {
  const { t } = useTranslation('messages');
  const media = useRef<HTMLMediaElement | null>(null);
  const [speed, setSpeed] = useState<(typeof SPEEDS)[number]>(1);
  const [playing, setPlaying] = useState(false);
  const [position, setPosition] = useState(0);
  const durationS = clip.duration_ms / 1000;

  useEffect(() => {
    const element = media.current;
    if (!element) return;
    element.defaultPlaybackRate = speed;
    element.playbackRate = speed;
  }, [speed]);

  const nextSpeed = () => setSpeed((current) => SPEEDS[(SPEEDS.indexOf(current) + 1) % SPEEDS.length]);
  const events = {
    onPlay: () => setPlaying(true),
    onPause: () => setPlaying(false),
    onEnded: () => setPlaying(false),
    onTimeUpdate: (event: { currentTarget: HTMLMediaElement }) =>
      setPosition(event.currentTarget.currentTime),
  };

  const bar = (
    <div className="flex items-center gap-2 text-xs text-muted">
      <span className="tabular-nums" data-qa="clip-duration">
        {clip.kind === 'audio' ? `${clipClock(position * 1000)} / ` : ''}
        {clipClock(clip.duration_ms)}
      </span>
      <button
        type="button"
        onClick={nextSpeed}
        aria-label={t('clips.player.speed', { rate: speed })}
        data-tooltip={t('clips.player.speed', { rate: speed })}
        data-qa="clip-speed"
        className="rounded px-1.5 py-0.5 font-semibold tabular-nums text-fg-dim hover:bg-raised"
      >
        {speed}×
      </button>
      <span className="min-w-0 flex-1 truncate" data-tooltip={filename}>
        {filename}
      </span>
      {onExpand && (
        <button
          type="button"
          onClick={onExpand}
          aria-label={t('clips.player.expand', { name: filename })}
          data-tooltip={t('clips.player.expand', { name: filename })}
          data-qa="clip-expand"
          className="shrink-0 rounded p-0.5 hover:bg-raised hover:text-fg-dim"
        >
          <Expand className="h-3.5 w-3.5" />
        </button>
      )}
      <a
        href={url}
        download={filename}
        aria-label={t('attachments.download', { name: filename })}
        className="shrink-0 rounded p-0.5 hover:bg-raised hover:text-fg-dim"
      >
        <Download className="h-3.5 w-3.5" />
      </a>
    </div>
  );

  if (clip.kind === 'video') {
    const [w, h] = width && height ? [width, height] : [16, 9];
    const fits = Math.round(Math.min(VIDEO_MAX_WIDTH, (VIDEO_MAX_HEIGHT * w) / h));
    return (
      <div
        className="flex w-full flex-col gap-1"
        style={{ maxWidth: fits }}
        data-qa="clip-player"
        data-kind="video"
      >
        <video
          ref={(element) => {
            media.current = element;
          }}
          src={url}
          poster={posterUrl ?? undefined}
          controls
          preload="metadata"
          playsInline
          style={{ aspectRatio: `${w} / ${h}` }}
          className="w-full rounded-lg border border-line/60 bg-black object-contain"
          {...events}
        />
        {bar}
      </div>
    );
  }

  const peaks = clip.waveform ?? [];
  const played = durationS > 0 ? Math.min(1, position / durationS) : 0;
  const seekTo = (seconds: number) => {
    const element = media.current;
    if (!element) return;
    element.currentTime = Math.max(0, Math.min(durationS, seconds));
    setPosition(element.currentTime);
  };
  const seekByPointer = (event: MouseEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    if (box.width > 0) seekTo(((event.clientX - box.left) / box.width) * durationS);
  };
  const seekByKey = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowRight') seekTo(position + SEEK_STEP_S);
    else if (event.key === 'ArrowLeft') seekTo(position - SEEK_STEP_S);
    else if (event.key === 'Home') seekTo(0);
    else if (event.key === 'End') seekTo(durationS);
    else return;
    event.preventDefault();
  };
  const toggle = () => {
    const element = media.current;
    if (!element) return;
    if (element.paused) void element.play().catch(() => undefined);
    else element.pause();
  };

  return (
    <div
      className="flex w-full max-w-sm flex-col gap-1.5 rounded-lg border border-line bg-surface px-3 py-2"
      data-qa="clip-player"
      data-kind="audio"
    >
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={toggle}
          aria-label={playing ? t('clips.player.pause') : t('clips.player.play')}
          data-tooltip={playing ? t('clips.player.pause') : t('clips.player.play')}
          data-qa="clip-play"
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-raised text-fg hover:bg-elevated"
        >
          {playing ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
        </button>
        <div
          role="slider"
          tabIndex={0}
          aria-label={t('clips.player.seek')}
          aria-valuemin={0}
          aria-valuemax={Math.round(durationS)}
          aria-valuenow={Math.round(position)}
          aria-valuetext={`${clipClock(position * 1000)} / ${clipClock(clip.duration_ms)}`}
          onClick={seekByPointer}
          onKeyDown={seekByKey}
          data-qa="clip-waveform"
          className="flex h-8 min-w-0 flex-1 cursor-pointer items-center gap-px rounded focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-purple-500"
        >
          {peaks.map((peak, index) => (
            <span
              key={index}
              className={`w-full rounded-full ${index / peaks.length < played ? 'bg-purple-500' : 'bg-elevated'}`}
              style={{ height: `${Math.max(8, peak)}%` }}
            />
          ))}
        </div>
      </div>
      {bar}
      <audio
        ref={(element) => {
          media.current = element;
        }}
        src={url}
        preload="metadata"
        className="hidden"
        {...events}
      />
    </div>
  );
}
