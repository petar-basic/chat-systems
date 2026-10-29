import { globalEventBus } from '@/lib/globalEventBus';
import { logger } from '@/lib/logger';
import type { HuddleTransport, TrackKind, TransportHandlers } from './HuddleTransport';

type SendFn = (msg: Record<string, unknown>) => void;

interface PeerState {
  pc: RTCPeerConnection;
  makingOffer: boolean;
  ignoreOffer: boolean;
  polite: boolean;
  pendingCandidates: RTCIceCandidateInit[];
  audioTx: RTCRtpTransceiver;
  cameraTx: RTCRtpTransceiver;
  screenTx: RTCRtpTransceiver;
}

const SLOT = { audio: 'audioTx', camera: 'cameraTx', screen: 'screenTx' } as const satisfies Record<
  TrackKind,
  keyof PeerState
>;

export class MeshManager implements HuddleTransport {
  readonly kind = 'mesh';
  private peers = new Map<string, PeerState>();
  private tracks: Record<TrackKind, MediaStreamTrack | null> = { audio: null, camera: null, screen: null };
  private muted = false;
  private readonly unsubscribe: (() => void)[];

  constructor(
    private readonly huddleId: string,
    private readonly selfUserId: string,
    private readonly iceServers: RTCIceServer[],
    private readonly send: SendFn,
    private readonly handlers: TransportHandlers,
  ) {
    this.unsubscribe = [
      globalEventBus.on('huddle.offer', (event) => {
        if (event.huddle_id === this.huddleId) void this.handleOffer(event.from_user_id, event.sdp);
      }),
      globalEventBus.on('huddle.answer', (event) => {
        if (event.huddle_id === this.huddleId) void this.handleAnswer(event.from_user_id, event.sdp);
      }),
      globalEventBus.on('huddle.ice', (event) => {
        if (event.huddle_id === this.huddleId) void this.handleCandidate(event.from_user_id, event.candidate);
      }),
    ];
  }

  setTrack(kind: TrackKind, track: MediaStreamTrack | null): void {
    this.tracks[kind] = track;
    if (kind === 'audio' && track) track.enabled = !this.muted;
    for (const peer of this.peers.values()) {
      void peer[SLOT[kind]].sender.replaceTrack(track);
    }
  }

  setMuted(muted: boolean): void {
    this.muted = muted;
    if (this.tracks.audio) this.tracks.audio.enabled = !muted;
  }

  addPeer(peerId: string): void {
    if (peerId === this.selfUserId || this.peers.has(peerId)) return;

    const pc = new RTCPeerConnection({ iceServers: this.iceServers });

    // Three m-lines up front, in an order both ends agree on: microphone,
    // camera, screen. A transceiver both sends and receives the same slot, so
    // the one that carries our camera carries theirs back, and `ontrack` can
    // name a stream by identity instead of guessing from the SDP. Every later
    // toggle is a replaceTrack into a slot that already exists, so turning a
    // camera on mid-call renegotiates nothing.
    const audioTx = pc.addTransceiver('audio', { direction: 'sendrecv' });
    const cameraTx = pc.addTransceiver('video', { direction: 'sendrecv' });
    const screenTx = pc.addTransceiver('video', { direction: 'sendrecv' });

    const state: PeerState = {
      pc,
      makingOffer: false,
      ignoreOffer: false,
      polite: this.selfUserId < peerId,
      pendingCandidates: [],
      audioTx,
      cameraTx,
      screenTx,
    };
    this.peers.set(peerId, state);

    void audioTx.sender.replaceTrack(this.tracks.audio);
    void cameraTx.sender.replaceTrack(this.tracks.camera);
    void screenTx.sender.replaceTrack(this.tracks.screen);

    pc.ontrack = (event) => {
      const kind: TrackKind =
        event.transceiver === cameraTx ? 'camera' : event.transceiver === screenTx ? 'screen' : 'audio';
      // A track handed over by replaceTrack carries no msid, so `event.streams`
      // is empty and the stream has to be built here.
      this.handlers.onTrack(peerId, new MediaStream([event.track]), kind);
    };

    pc.onicecandidate = ({ candidate }) => {
      if (candidate) {
        this.send({
          type: 'huddle.ice',
          huddle_id: this.huddleId,
          to_user_id: peerId,
          candidate: candidate.toJSON(),
        });
      }
    };

    pc.onnegotiationneeded = async () => {
      try {
        state.makingOffer = true;
        await pc.setLocalDescription();
        this.send({
          type: 'huddle.offer',
          huddle_id: this.huddleId,
          to_user_id: peerId,
          sdp: pc.localDescription,
        });
      } catch (err) {
        logger.error('MeshManager', 'negotiationneeded', err);
      } finally {
        state.makingOffer = false;
      }
    };

    pc.onconnectionstatechange = () => {
      logger.info('MeshManager', 'connectionState', `${peerId} -> ${pc.connectionState}`);
      if (pc.connectionState === 'failed') pc.restartIce();
    };
  }

  private async flushCandidates(state: PeerState): Promise<void> {
    const pending = state.pendingCandidates;
    state.pendingCandidates = [];
    for (const candidate of pending) {
      try {
        await state.pc.addIceCandidate(candidate);
      } catch (err) {
        logger.error('MeshManager', 'flushCandidates', err);
      }
    }
  }

  private async handleOffer(peerId: string, sdp: RTCSessionDescriptionInit): Promise<void> {
    if (!this.peers.has(peerId)) this.addPeer(peerId);
    const state = this.peers.get(peerId);
    if (!state) return;
    const { pc } = state;

    const offerCollision = state.makingOffer || pc.signalingState !== 'stable';
    state.ignoreOffer = !state.polite && offerCollision;
    if (state.ignoreOffer) return;

    await pc.setRemoteDescription(sdp);
    await this.flushCandidates(state);
    await pc.setLocalDescription();
    this.send({
      type: 'huddle.answer',
      huddle_id: this.huddleId,
      to_user_id: peerId,
      sdp: pc.localDescription,
    });
  }

  private async handleAnswer(peerId: string, sdp: RTCSessionDescriptionInit): Promise<void> {
    const state = this.peers.get(peerId);
    if (!state || state.pc.signalingState === 'stable') return;
    await state.pc.setRemoteDescription(sdp);
    await this.flushCandidates(state);
  }

  private async handleCandidate(peerId: string, candidate: RTCIceCandidateInit): Promise<void> {
    const state = this.peers.get(peerId);
    if (!state) return;
    if (!state.pc.remoteDescription) {
      state.pendingCandidates.push(candidate);
      return;
    }
    try {
      await state.pc.addIceCandidate(candidate);
    } catch (err) {
      if (!state.ignoreOffer) logger.error('MeshManager', 'addIceCandidate', err);
    }
  }

  removePeer(peerId: string): void {
    const state = this.peers.get(peerId);
    if (!state) return;
    state.pc.ontrack = null;
    state.pc.onicecandidate = null;
    state.pc.onnegotiationneeded = null;
    state.pc.onconnectionstatechange = null;
    state.pc.close();
    this.peers.delete(peerId);
  }

  restart(): void {
    for (const peerId of [...this.peers.keys()]) this.removePeer(peerId);
  }

  close(): void {
    this.restart();
    for (const off of this.unsubscribe) off();
  }
}
