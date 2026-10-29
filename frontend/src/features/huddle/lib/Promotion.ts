import type { HuddleTransport, TrackKind, TransportHandlers, TransportKind } from './HuddleTransport';

export const HANDOVER_TIMEOUT_MS = 8000;

export type LocalMedia = Record<TrackKind, MediaStreamTrack | null>;

export class Promotion implements HuddleTransport {
  private sfu: HuddleTransport | null = null;
  private live = false;
  private closed = false;
  private readonly waiting: Set<string>;
  private readonly tracks: LocalMedia;
  private readonly held = new Map<string, Map<TrackKind, MediaStream | null>>();
  private release: (() => void) | null = null;

  constructor(
    private readonly mesh: HuddleTransport,
    peers: Iterable<string>,
    private readonly handlers: TransportHandlers,
    private muted: boolean,
    tracks: LocalMedia,
  ) {
    this.waiting = new Set(peers);
    this.tracks = { ...tracks };
  }

  get kind(): TransportKind {
    return this.live ? 'sfu' : 'mesh';
  }

  async run(
    open: (handlers: TransportHandlers) => Promise<HuddleTransport>,
    timeoutMs = HANDOVER_TIMEOUT_MS,
  ): Promise<void> {
    const sfu = await open({
      onTrack: (peerId, stream, kind) => this.sfuTrack(peerId, stream, kind),
      onSpeakers: (userIds) => {
        if (this.live) this.handlers.onSpeakers(userIds);
      },
    });
    if (this.closed) {
      sfu.close();
      return;
    }
    this.sfu = sfu;
    sfu.setMuted(this.muted);
    for (const kind of Object.keys(this.tracks) as TrackKind[]) sfu.setTrack(kind, this.tracks[kind]);

    await this.everyoneHeard(timeoutMs);
    if (!this.closed) this.handOver();
  }

  setTrack(kind: TrackKind, track: MediaStreamTrack | null): void {
    this.tracks[kind] = track;
    if (!this.live) this.mesh.setTrack(kind, track);
    this.sfu?.setTrack(kind, track);
  }

  setMuted(muted: boolean): void {
    this.muted = muted;
    if (!this.live) this.mesh.setMuted(muted);
    this.sfu?.setMuted(muted);
  }

  addPeer(peerId: string): void {
    this.sfu?.addPeer(peerId);
  }

  removePeer(peerId: string): void {
    if (!this.live) this.mesh.removePeer(peerId);
    this.sfu?.removePeer(peerId);
    this.heard(peerId);
  }

  restart(): void {
    if (!this.live) this.mesh.restart();
    this.sfu?.restart();
  }

  close(): void {
    this.closed = true;
    this.release?.();
    this.mesh.close();
    this.sfu?.close();
  }

  private sfuTrack(peerId: string, stream: MediaStream | null, kind: TrackKind): void {
    if (this.live) {
      this.handlers.onTrack(peerId, stream, kind);
      return;
    }
    const tracks = this.held.get(peerId) ?? new Map<TrackKind, MediaStream | null>();
    tracks.set(kind, stream);
    this.held.set(peerId, tracks);
    if (kind === 'audio' && stream) this.heard(peerId);
  }

  private heard(peerId: string): void {
    this.waiting.delete(peerId);
    if (this.waiting.size === 0) this.release?.();
  }

  private everyoneHeard(timeoutMs: number): Promise<void> {
    if (this.waiting.size === 0) return Promise.resolve();
    return new Promise((resolve) => {
      const timer = setTimeout(() => this.release?.(), timeoutMs);
      this.release = () => {
        clearTimeout(timer);
        this.release = null;
        resolve();
      };
    });
  }

  private handOver(): void {
    this.live = true;
    this.mesh.close();
    for (const [peerId, tracks] of this.held) {
      for (const [kind, stream] of tracks) this.handlers.onTrack(peerId, stream, kind);
    }
    this.held.clear();
  }
}
