import { useEffect } from 'react';
import { useHuddleStore } from '@/stores/huddle';
import { useSpeaking } from '../hooks/useSpeaking';
import { VideoTile } from './VideoTile';
import { focusTileOf, type TileProps } from './huddleTiles';

export type Layout = 'grid' | 'focus';

interface Props {
  tiles: TileProps[];
  self: string;
  layout: Layout;
  expanded: boolean;
}

export function HuddleStage({ tiles, self, layout, expanded }: Props) {
  const speaking = useHuddleStore((s) => s.speaking);
  const pinnedUserId = useHuddleStore((s) => s.pinnedUserId);
  const speakerId = useHuddleStore((s) => s.devices.speakerId);
  const reactions = useHuddleStore((s) => s.reactions);
  const detectSpeaking = useHuddleStore((s) => s.transport === 'mesh');

  const focusTile = focusTileOf(tiles, self, speaking, pinnedUserId);
  const stripTiles = tiles.filter((t) => t.tileId !== focusTile.tileId);

  return (
    <>
      {detectSpeaking &&
        tiles
          .filter((t) => t.audioStream)
          .map((t) => <SpeakingDetector key={`spk-${t.tileId}`} userId={t.userId} stream={t.audioStream} />)}

      <div className={`relative ${expanded ? 'flex-1 min-h-0 overflow-y-auto' : ''}`}>
        {layout === 'focus' ? (
          <div className="flex flex-col gap-2">
            <VideoTile
              {...focusTile}
              speaking={speaking.has(focusTile.userId)}
              pinned={pinnedUserId === focusTile.userId}
              speakerId={speakerId}
              large
            />
            {stripTiles.length > 0 && (
              <div className="flex gap-2 overflow-x-auto">
                {stripTiles.map((t) => (
                  <div key={t.tileId} className="w-28 shrink-0">
                    <VideoTile
                      {...t}
                      speaking={speaking.has(t.userId)}
                      pinned={pinnedUserId === t.userId}
                      speakerId={speakerId}
                    />
                  </div>
                ))}
              </div>
            )}
          </div>
        ) : (
          <div
            className={`grid gap-2 ${expanded ? 'grid-cols-2 md:grid-cols-3 lg:grid-cols-4 content-start' : 'grid-cols-2'}`}
          >
            {tiles.map((t) => (
              <VideoTile
                key={t.tileId}
                {...t}
                speaking={speaking.has(t.userId)}
                pinned={pinnedUserId === t.userId}
                speakerId={speakerId}
              />
            ))}
          </div>
        )}

        {reactions.length > 0 && (
          <div className="pointer-events-none absolute inset-x-0 bottom-2 flex justify-center gap-2">
            {reactions.map((r) => (
              <span key={r.id} className="huddle-float text-3xl drop-shadow-lg">
                {r.emoji}
              </span>
            ))}
          </div>
        )}
      </div>
    </>
  );
}

function SpeakingDetector({ userId, stream }: { userId: string; stream: MediaStream | null }) {
  const speaking = useSpeaking(stream);
  useEffect(() => {
    useHuddleStore.getState().setSpeaking(userId, speaking);
  }, [userId, speaking]);
  return null;
}
