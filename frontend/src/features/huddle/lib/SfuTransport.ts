import { Room, RoomEvent, Track, type RemoteTrack, type RemoteTrackPublication } from 'livekit-client';
import { logger } from '@/lib/logger';
import type { HuddleTransport, TrackKind, TransportHandlers } from './HuddleTransport';

const SOURCE = {
  audio: Track.Source.Microphone,
  camera: Track.Source.Camera,
  screen: Track.Source.ScreenShare,
} as const satisfies Record<TrackKind, Track.Source>;

const kindOf = (source: Track.Source): TrackKind | null =>
  source === Track.Source.Microphone
    ? 'audio'
    : source === Track.Source.Camera
      ? 'camera'
      : source === Track.Source.ScreenShare
        ? 'screen'
        : null;

export interface SfuRoom {
  url: string;
  token: string;
}

export class SfuTransport implements HuddleTransport {
  readonly kind = 'sfu';
  private readonly room = new Room({ adaptiveStream: false, dynacast: true });
  private tracks: Record<TrackKind, MediaStreamTrack | null> = { audio: null, camera: null, screen: null };
  private muted = false;
  private connected = false;
  private pending: Promise<void> = Promise.resolve();

  constructor(
    private readonly target: SfuRoom,
    private readonly iceServers: RTCIceServer[],
    private readonly handlers: TransportHandlers,
  ) {
    this.room
      .on(RoomEvent.TrackSubscribed, (track, publication, participant) =>
        this.announce(participant.identity, publication, track),
      )
      .on(RoomEvent.TrackUnsubscribed, (_track, publication, participant) =>
        this.announce(participant.identity, publication, null),
      )
      .on(RoomEvent.ActiveSpeakersChanged, (speakers) =>
        this.handlers.onSpeakers(speakers.map((p) => p.identity)),
      );
  }

  async join(): Promise<void> {
    await this.room.connect(this.target.url, this.target.token, {
      autoSubscribe: true,
      rtcConfig: { iceServers: this.iceServers },
    });
    this.connected = true;
    for (const kind of Object.keys(SOURCE) as TrackKind[]) this.enqueue(() => this.publish(kind));
    await this.pending;
  }

  setTrack(kind: TrackKind, track: MediaStreamTrack | null): void {
    this.tracks[kind] = track;
    this.enqueue(() => this.publish(kind));
  }

  setMuted(muted: boolean): void {
    this.muted = muted;
    this.enqueue(() => this.applyMute());
  }

  addPeer(): void {}

  removePeer(): void {}

  restart(): void {
    for (const participant of this.room.remoteParticipants.values()) {
      for (const publication of participant.trackPublications.values()) {
        if (publication.track) this.announce(participant.identity, publication, publication.track);
      }
    }
  }

  close(): void {
    this.connected = false;
    this.room.removeAllListeners();
    void this.room.disconnect(false);
  }

  private announce(identity: string, publication: RemoteTrackPublication, track: RemoteTrack | null): void {
    const kind = kindOf(publication.source);
    if (!kind) return;
    this.handlers.onTrack(identity, track ? new MediaStream([track.mediaStreamTrack]) : null, kind);
  }

  private enqueue(step: () => Promise<void>): void {
    this.pending = this.pending
      .then(step)
      .catch((err: unknown) => logger.error('SfuTransport', 'publish', err));
  }

  private async publish(kind: TrackKind): Promise<void> {
    if (!this.connected) return;
    const local = this.room.localParticipant;
    const track = this.tracks[kind];
    const published = local.getTrackPublication(SOURCE[kind])?.track;
    if (!track) {
      if (published) await local.unpublishTrack(published, false);
      return;
    }
    if (kind === 'audio') track.enabled = !this.muted;
    if (!published) {
      await local.publishTrack(track, { source: SOURCE[kind] });
    } else if (published.mediaStreamTrack !== track) {
      await published.replaceTrack(track, { userProvidedTrack: true });
    }
    if (kind === 'audio') await this.applyMute();
  }

  private async applyMute(): Promise<void> {
    const microphone = this.room.localParticipant.getTrackPublication(Track.Source.Microphone)?.track;
    if (!microphone) return;
    if (this.muted) await microphone.mute();
    else await microphone.unmute();
  }
}
