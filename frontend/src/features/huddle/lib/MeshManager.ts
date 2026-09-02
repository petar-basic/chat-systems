import { logger } from '@/lib/logger';

export type TrackKind = 'audio' | 'camera' | 'screen';

type SendFn = (msg: Record<string, unknown>) => void;
type TrackHandler = (peerId: string, stream: MediaStream, kind: TrackKind) => void;

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

export class MeshManager {
  private peers = new Map<string, PeerState>();
  private audioTrack: MediaStreamTrack | null = null;
  private cameraTrack: MediaStreamTrack | null = null;
  private screenTrack: MediaStreamTrack | null = null;

  constructor(
    private readonly huddleId: string,
    private readonly selfUserId: string,
    private readonly iceServers: RTCIceServer[],
    private readonly send: SendFn,
    private readonly onTrack: TrackHandler,
  ) {}

  setAudioTrack(track: MediaStreamTrack | null): void {
    this.audioTrack = track;
    for (const { audioTx } of this.peers.values()) {
      void audioTx.sender.replaceTrack(track);
    }
  }

  setCameraTrack(track: MediaStreamTrack | null): void {
    this.cameraTrack = track;
    for (const { cameraTx } of this.peers.values()) {
      void cameraTx.sender.replaceTrack(track);
    }
  }

  setScreenTrack(track: MediaStreamTrack | null): void {
    this.screenTrack = track;
    for (const { screenTx } of this.peers.values()) {
      void screenTx.sender.replaceTrack(track);
    }
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

    void audioTx.sender.replaceTrack(this.audioTrack);
    void cameraTx.sender.replaceTrack(this.cameraTrack);
    void screenTx.sender.replaceTrack(this.screenTrack);

    pc.ontrack = (event) => {
      const kind: TrackKind =
        event.transceiver === cameraTx ? 'camera' : event.transceiver === screenTx ? 'screen' : 'audio';
      // A track handed over by replaceTrack carries no msid, so `event.streams`
      // is empty and the stream has to be built here.
      this.onTrack(peerId, new MediaStream([event.track]), kind);
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

  async handleOffer(peerId: string, sdp: RTCSessionDescriptionInit): Promise<void> {
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

  async handleAnswer(peerId: string, sdp: RTCSessionDescriptionInit): Promise<void> {
    const state = this.peers.get(peerId);
    if (!state || state.pc.signalingState === 'stable') return;
    await state.pc.setRemoteDescription(sdp);
    await this.flushCandidates(state);
  }

  async handleCandidate(peerId: string, candidate: RTCIceCandidateInit): Promise<void> {
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

  close(): void {
    for (const peerId of [...this.peers.keys()]) this.removePeer(peerId);
  }
}
