import type { HuddleParticipant } from '@/stores/huddle';

export interface TileProps {
  tileId: string;
  userId: string;
  kind: 'camera' | 'screen';
  stream: MediaStream | null;
  audioStream: MediaStream | null;
  muted: boolean;
  cameraOn: boolean;
  sharing: boolean;
  handRaised: boolean;
  isSelf: boolean;
}

interface Local {
  self: string;
  videoStream: MediaStream | null;
  screenStream: MediaStream | null;
  muted: boolean;
  cameraOn: boolean;
  sharing: boolean;
  handRaised: boolean;
}

export function buildTiles(local: Local, remotes: HuddleParticipant[]): TileProps[] {
  const { self } = local;
  // A shared screen is its own tile, not a replacement for the face: whoever is
  // sharing shows up twice, once as themselves and once as their screen.
  return [
    {
      tileId: `${self}:camera`,
      userId: self,
      kind: 'camera' as const,
      stream: local.videoStream,
      audioStream: null,
      muted: local.muted,
      cameraOn: local.cameraOn,
      sharing: local.sharing,
      handRaised: local.handRaised,
      isSelf: true,
    },
    ...(local.sharing && local.screenStream
      ? [
          {
            tileId: `${self}:screen`,
            userId: self,
            kind: 'screen' as const,
            stream: local.screenStream,
            audioStream: null,
            muted: local.muted,
            cameraOn: local.cameraOn,
            sharing: true,
            handRaised: false,
            isSelf: true,
          },
        ]
      : []),
    ...remotes.flatMap((p) => [
      {
        tileId: `${p.userId}:camera`,
        userId: p.userId,
        kind: 'camera' as const,
        stream: p.cameraStream,
        audioStream: p.audioStream,
        muted: p.audioMuted,
        cameraOn: p.cameraOn,
        sharing: p.sharing,
        handRaised: p.handRaised,
        isSelf: false,
      },
      ...(p.sharing && p.screenStream
        ? [
            {
              tileId: `${p.userId}:screen`,
              userId: p.userId,
              kind: 'screen' as const,
              stream: p.screenStream,
              audioStream: null,
              muted: p.audioMuted,
              cameraOn: p.cameraOn,
              sharing: true,
              handRaised: false,
              isSelf: false,
            },
          ]
        : []),
    ]),
  ];
}

export function focusTileOf(
  tiles: TileProps[],
  self: string,
  speaking: Set<string>,
  pinnedUserId: string | null,
): TileProps {
  // A screen is shared to be looked at, so it takes the stage over whoever
  // happens to be talking; an explicit pin still wins over both.
  const activeSpeaker = [...speaking].find((id) => id !== self) ?? null;
  const firstRemote = tiles.find((t) => !t.isSelf)?.userId;
  const sharedScreen = tiles.find((t) => t.kind === 'screen' && !t.isSelf) ?? null;
  const pinnedTile = pinnedUserId ? tiles.find((t) => t.userId === pinnedUserId) : null;
  return (
    pinnedTile ??
    sharedScreen ??
    tiles.find((t) => t.userId === (activeSpeaker ?? firstRemote ?? self)) ??
    tiles[0]
  );
}
