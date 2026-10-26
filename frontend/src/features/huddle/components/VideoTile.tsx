import { useCallback, useEffect, useRef, useSyncExternalStore } from 'react';
import { MicOff, Pin } from 'lucide-react';
import { useHuddleStore } from '@/stores/huddle';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { logger } from '@/lib/logger';
import type { TileProps } from './huddleTiles';

type VideoTileProps = TileProps & {
  speaking: boolean;
  pinned: boolean;
  speakerId: string | null;
  large?: boolean;
};

type SinkAudio = HTMLAudioElement & { setSinkId?: (id: string) => Promise<void> };

export function VideoTile({
  userId,
  kind,
  stream,
  audioStream,
  muted,
  sharing,
  handRaised,
  isSelf,
  speaking,
  pinned,
  speakerId,
  large,
}: VideoTileProps) {
  const { getUser } = useUserCache();
  const participant = getUser(userId);
  const name = displayNameOf(participant?.display_name);
  const videoRef = useRef<HTMLVideoElement>(null);
  const audioRef = useRef<HTMLAudioElement>(null);
  const isScreen = kind === 'screen';

  const subscribeVideo = useCallback(
    (onChange: () => void) => {
      if (!stream) return () => undefined;
      const subscribed = new Set<MediaStreamTrack>();
      const sync = () => {
        for (const t of stream.getVideoTracks()) {
          if (!subscribed.has(t)) {
            subscribed.add(t);
            t.addEventListener('mute', onChange);
            t.addEventListener('unmute', onChange);
            t.addEventListener('ended', onChange);
          }
        }
      };
      const onMutation = () => {
        sync();
        onChange();
      };
      sync();
      stream.addEventListener('addtrack', onMutation);
      stream.addEventListener('removetrack', onMutation);
      return () => {
        stream.removeEventListener('addtrack', onMutation);
        stream.removeEventListener('removetrack', onMutation);
        for (const t of subscribed) {
          t.removeEventListener('mute', onChange);
          t.removeEventListener('unmute', onChange);
          t.removeEventListener('ended', onChange);
        }
      };
    },
    [stream],
  );
  const hasVideo = useSyncExternalStore(
    subscribeVideo,
    () => !!stream && stream.getVideoTracks().some((t) => t.readyState === 'live' && !t.muted),
  );

  useEffect(() => {
    const v = videoRef.current;
    if (v && v.srcObject !== stream) v.srcObject = stream;
  }, [stream]);

  useEffect(() => {
    const a = audioRef.current;
    if (a && a.srcObject !== audioStream) {
      a.srcObject = audioStream;
      void a.play().catch((e) => logger.warn('VideoTile', 'audioPlay', String(e)));
    }
  }, [audioStream]);

  useEffect(() => {
    if (hasVideo)
      void videoRef.current?.play().catch((e) => logger.warn('VideoTile', 'videoPlay', String(e)));
  }, [hasVideo]);

  useEffect(() => {
    const a = audioRef.current as SinkAudio | null;
    if (a?.setSinkId && speakerId) {
      void a.setSinkId(speakerId).catch((e) => logger.warn('VideoTile', 'setSinkId', String(e)));
    }
  }, [speakerId]);

  return (
    <div
      className={`relative ${large ? 'aspect-video' : 'aspect-[4/3]'} rounded-lg overflow-hidden bg-app ring-2 transition ${
        speaking && !muted ? 'ring-green-500/70' : 'ring-transparent'
      }`}
    >
      <video
        ref={videoRef}
        autoPlay
        playsInline
        muted
        className={`w-full h-full ${isScreen ? 'object-contain bg-black' : 'object-cover'} ${
          hasVideo ? '' : 'hidden'
        } ${isSelf && !isScreen ? 'scale-x-[-1]' : ''}`}
      />
      {!isSelf && audioStream && <audio ref={audioRef} autoPlay />}
      {!hasVideo &&
        (isScreen ? (
          <div className="absolute inset-0 flex items-center justify-center text-xs text-muted">
            Waiting for the screen…
          </div>
        ) : (
          <div className="absolute inset-0 flex items-center justify-center">
            <Avatar
              userId={userId}
              name={name}
              avatarUrl={participant?.avatar_url}
              size={large ? 'xl' : 'lg'}
            />
          </div>
        ))}

      {handRaised && (
        <div className="absolute top-1 left-1 text-lg" aria-label="Hand raised" title="Hand raised">
          ✋
        </div>
      )}

      <div className="absolute bottom-1 left-1 right-1 flex items-center gap-1">
        <span className="px-1.5 py-0.5 rounded bg-black/50 text-[11px] text-white truncate max-w-full">
          {isScreen ? `${name}'s screen` : name}
          {isSelf && !isScreen && ' (you)'}
        </span>
        {sharing && !isScreen && (
          <span className="px-1 py-0.5 rounded bg-purple-600/80 text-[10px] text-fg">Sharing</span>
        )}
        {muted && !isScreen && <MicOff className="w-3 h-3 text-danger shrink-0" />}
      </div>

      {!isSelf && !isScreen && (
        <button
          onClick={() => useHuddleStore.getState().setPinned(pinned ? null : userId)}
          aria-label={pinned ? 'Unpin' : 'Pin'}
          title={pinned ? 'Unpin' : 'Pin'}
          className={`absolute top-1 right-1 p-1 rounded-md transition ${
            pinned ? 'bg-purple-600 text-white' : 'bg-black/40 text-slate-200'
          }`}
        >
          <Pin className="w-3 h-3" />
        </button>
      )}
    </div>
  );
}
