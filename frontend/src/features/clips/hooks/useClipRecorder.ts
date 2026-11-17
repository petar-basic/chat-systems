import { useCallback, useEffect, useRef, useState } from 'react';
import i18n from '@/lib/i18n';
import { logger } from '@/lib/logger';
import { CLIP_MAX_MS, type ClipMode } from '../lib/format';
import { finishClip, releaseClip, type RecordedClip } from '../lib/finish';
import { ClipDenied, ClipSession } from '../lib/session';

const CLOCK_EVERY_MS = 250;

export type RecorderPhase =
  | { phase: 'choosing' }
  | { phase: 'starting' }
  | { phase: 'recording'; elapsedMs: number; preview: MediaStream | null }
  | { phase: 'processing' }
  | { phase: 'ready'; clip: RecordedClip }
  | { phase: 'failed'; error: string };

export function useClipRecorder() {
  const [state, setState] = useState<RecorderPhase>({ phase: 'choosing' });
  const session = useRef<ClipSession | null>(null);
  const mode = useRef<ClipMode>('screen');
  const ready = useRef<RecordedClip | null>(null);
  const alive = useRef(true);

  const stop = useCallback(async () => {
    const current = session.current;
    if (!current) return;
    session.current = null;
    setState({ phase: 'processing' });
    try {
      const recorded = await current.stop();
      const clip = await finishClip(recorded, mode.current, new Date());
      if (!alive.current) {
        releaseClip(clip);
        return;
      }
      ready.current = clip;
      setState({ phase: 'ready', clip });
    } catch (err) {
      logger.error('clips', 'stop', err);
      if (alive.current) {
        setState({
          phase: 'failed',
          error: err instanceof ClipDenied ? err.message : i18n.t('messages:clips.errors.failed'),
        });
      }
    }
  }, []);

  const start = useCallback(
    async (picked: ClipMode, bubble: boolean) => {
      mode.current = picked;
      setState({ phase: 'starting' });
      try {
        const started = await ClipSession.start(picked, bubble, () => void stop());
        if (!alive.current) {
          started.cancel();
          return;
        }
        session.current = started;
        setState({ phase: 'recording', elapsedMs: 0, preview: started.preview });
      } catch (err) {
        if (!(err instanceof ClipDenied)) logger.error('clips', 'start', err);
        setState({
          phase: 'failed',
          error: err instanceof ClipDenied ? err.message : i18n.t('messages:clips.errors.failed'),
        });
      }
    },
    [stop],
  );

  const recording = state.phase === 'recording';
  useEffect(() => {
    if (!recording) return undefined;
    const clock = window.setInterval(() => {
      const current = session.current;
      if (!current) return;
      const elapsedMs = current.elapsedMs;
      if (elapsedMs >= CLIP_MAX_MS) {
        void stop();
        return;
      }
      setState((prev) => (prev.phase === 'recording' ? { ...prev, elapsedMs } : prev));
    }, CLOCK_EVERY_MS);
    const limit = window.setTimeout(() => void stop(), CLIP_MAX_MS);
    return () => {
      window.clearInterval(clock);
      window.clearTimeout(limit);
    };
  }, [recording, stop]);

  const again = useCallback(() => {
    if (ready.current) releaseClip(ready.current);
    ready.current = null;
    setState({ phase: 'choosing' });
  }, []);

  const take = useCallback((): RecordedClip | null => {
    const clip = ready.current;
    ready.current = null;
    return clip;
  }, []);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      session.current?.cancel();
      session.current = null;
      if (ready.current) releaseClip(ready.current);
      ready.current = null;
    };
  }, []);

  return { state, start, stop, again, take };
}
