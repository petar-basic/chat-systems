import { logger } from '@/lib/logger';

type SendFn = (msg: Record<string, unknown>) => void;
type TrackHandler = (peerId: string, stream: MediaStream) => void;

interface PeerState {
  pc: RTCPeerConnection;
  makingOffer: boolean;
  ignoreOffer: boolean;
  polite: boolean;
  pendingCandidates: RTCIceCandidateInit[];
}

export class MeshManager {
  private peers = new Map<string, PeerState>();
  private localStream: MediaStream | null = null;
  private videoTrack: MediaStreamTrack | null = null;
  private videoSenders = new Map<string, RTCRtpSender>();

  constructor(
    private readonly huddleId: string,
    private readonly selfUserId: string,
    private readonly iceServers: RTCIceServer[],
    private readonly send: SendFn,
    private readonly onTrack: TrackHandler,
  ) {}

  setLocalStream(stream: MediaStream): void {
    this.localStream = stream;
    for (const { pc } of this.peers.values()) {
      this.attachLocalTracks(pc);
    }
  }

  setVideoTrack(track: MediaStreamTrack | null): void {
    this.videoTrack = track;
    for (const [peerId, { pc }] of this.peers) {
      this.applyVideo(peerId, pc);
    }
  }

  setAudioTrack(track: MediaStreamTrack): void {
    for (const { pc } of this.peers.values()) {
      const sender = pc.getSenders().find((s) => s.track?.kind === 'audio');
      if (sender) void sender.replaceTrack(track);
    }
  }

  addPeer(peerId: string): void {
    if (peerId === this.selfUserId || this.peers.has(peerId)) return;

    const pc = new RTCPeerConnection({ iceServers: this.iceServers });
    const state: PeerState = {
      pc,
      makingOffer: false,
      ignoreOffer: false,
      polite: this.selfUserId < peerId,
      pendingCandidates: [],
    };
    this.peers.set(peerId, state);

    this.attachLocalTracks(pc);

    pc.ontrack = (event) => {
      const [stream] = event.streams;
      if (stream) this.onTrack(peerId, stream);
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

    this.applyVideo(peerId, pc);
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

  private applyVideo(peerId: string, pc: RTCPeerConnection): void {
    const existing = this.videoSenders.get(peerId);
    if (existing) {
      void existing.replaceTrack(this.videoTrack);
    } else if (this.videoTrack && this.localStream) {
      const sender = pc.addTrack(this.videoTrack, this.localStream);
      this.videoSenders.set(peerId, sender);
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
    this.videoSenders.delete(peerId);
  }

  close(): void {
    for (const peerId of [...this.peers.keys()]) this.removePeer(peerId);
  }

  private attachLocalTracks(pc: RTCPeerConnection): void {
    if (!this.localStream) return;
    const senderTracks = new Set(pc.getSenders().map((s) => s.track));
    for (const track of this.localStream.getTracks()) {
      if (!senderTracks.has(track)) pc.addTrack(track, this.localStream);
    }
  }
}
