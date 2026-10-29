export type TrackKind = 'audio' | 'camera' | 'screen';

export type TransportKind = 'mesh' | 'sfu';

export interface TransportHandlers {
  onTrack: (peerId: string, stream: MediaStream | null, kind: TrackKind) => void;
  onSpeakers: (userIds: string[]) => void;
}

export interface HuddleTransport {
  readonly kind: TransportKind;
  setTrack(kind: TrackKind, track: MediaStreamTrack | null): void;
  setMuted(muted: boolean): void;
  addPeer(peerId: string): void;
  removePeer(peerId: string): void;
  restart(): void;
  close(): void;
}
