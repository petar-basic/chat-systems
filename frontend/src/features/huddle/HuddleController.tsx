import { useCallback, useEffect, useRef } from 'react';
import { globalEventBus } from '@/lib/globalEventBus';
import { instanceManager } from '@/lib/instances';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { useWorkspaceStore } from '@/stores/workspace';
import { useHuddleStore, type ActiveHuddle } from '@/stores/huddle';
import { logger } from '@/lib/logger';
import { MeshManager } from './lib/MeshManager';
import { acquireCamera, acquireLocalAudio, acquireScreen, stopStream } from './lib/media';
import { BackgroundProcessor } from './lib/backgroundProcessor';
import { HuddleWindow } from './components/HuddleWindow';
import { IncomingCallRing } from './components/IncomingCallRing';

export interface HuddleControls {
  toggleMute: () => void;
  toggleCamera: () => void;
  toggleScreen: () => void;
  toggleHand: () => void;
  toggleBackground: () => void;
  sendReaction: (emoji: string) => void;
  leave: () => void;
  selectMic: (deviceId: string) => void;
  selectCamera: (deviceId: string) => void;
  selectSpeaker: (deviceId: string) => void;
}

const wsFor = (active: ActiveHuddle) => instanceManager.get(active.instanceUrl).ws;

export function HuddleController() {
  const active = useHuddleStore((s) => s.active);
  const incomingCalls = useHuddleStore((s) => s.incomingCalls);

  const meshRef = useRef<MeshManager | null>(null);
  const micStreamRef = useRef<MediaStream | null>(null);
  const camTrackRef = useRef<MediaStreamTrack | null>(null);
  const screenTrackRef = useRef<MediaStreamTrack | null>(null);
  const processedTrackRef = useRef<MediaStreamTrack | null>(null);
  const bgProcessorRef = useRef<BackgroundProcessor | null>(null);

  useEffect(() => {
    return globalEventBus.on('huddle.ring', (event) => {
      const store = useHuddleStore.getState();
      if (store.active?.huddleId === event.huddle_id) return;
      const instanceUrl = useWorkspaceStore.getState().currentWorkspace?.instanceUrl;
      if (!instanceUrl) return;
      store.addIncomingCall({
        huddleId: event.huddle_id,
        fromUserId: event.from_user_id,
        workspaceId: event.workspace_id,
        instanceUrl,
      });
    });
  }, []);

  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    const ws = wsFor(active);
    const send = (msg: Record<string, unknown>) => ws.send(msg);

    const start = async () => {
      let iceServers: RTCIceServer[] = [];
      try {
        const res = await getApiForInstance(active.instanceUrl).get<{ ice_servers: RTCIceServer[] }>(
          `/workspaces/${active.workspaceId}/ice-servers`,
        );
        iceServers = res.ice_servers ?? [];
      } catch (err) {
        logger.error('HuddleController', 'ice-servers', err);
      }
      if (cancelled) return;

      let stream: MediaStream;
      try {
        stream = await acquireLocalAudio(useHuddleStore.getState().devices.micId);
      } catch (err) {
        logger.error('HuddleController', 'getUserMedia', err);
        useHuddleStore.getState().setActive(null);
        return;
      }
      if (cancelled) {
        stopStream(stream);
        return;
      }

      micStreamRef.current = stream;
      useHuddleStore.getState().setLocalStream(stream);

      const mesh = new MeshManager(active.huddleId, active.selfUserId, iceServers, send, (peerId, remote) => {
        const store = useHuddleStore.getState();
        store.upsertParticipant(peerId);
        store.setParticipantStream(peerId, remote);
      });
      mesh.setLocalStream(stream);
      meshRef.current = mesh;

      send(
        active.scope.kind === 'channel'
          ? { type: 'huddle.join', huddle_id: active.huddleId, channel_id: active.scope.channelId }
          : {
              type: 'huddle.join',
              huddle_id: active.huddleId,
              workspace_id: active.workspaceId,
              dm_partner_id: active.scope.partnerId,
            },
      );
    };

    void start();

    const unsubs = [
      globalEventBus.on('huddle.members', (event) => {
        if (event.huddle_id !== active.huddleId) return;
        for (const uid of event.user_ids) {
          if (uid === active.selfUserId) continue;
          useHuddleStore.getState().upsertParticipant(uid);
          meshRef.current?.addPeer(uid);
        }
      }),
      globalEventBus.on('huddle.member_joined', (event) => {
        if (event.huddle_id !== active.huddleId || event.user_id === active.selfUserId) return;
        useHuddleStore.getState().upsertParticipant(event.user_id);
        meshRef.current?.addPeer(event.user_id);
      }),
      globalEventBus.on('huddle.member_left', (event) => {
        if (event.huddle_id !== active.huddleId) return;
        meshRef.current?.removePeer(event.user_id);
        useHuddleStore.getState().removeParticipant(event.user_id);
      }),
      globalEventBus.on('huddle.offer', (event) => {
        if (event.huddle_id !== active.huddleId) return;
        void meshRef.current?.handleOffer(event.from_user_id, event.sdp);
      }),
      globalEventBus.on('huddle.answer', (event) => {
        if (event.huddle_id !== active.huddleId) return;
        void meshRef.current?.handleAnswer(event.from_user_id, event.sdp);
      }),
      globalEventBus.on('huddle.ice', (event) => {
        if (event.huddle_id !== active.huddleId) return;
        void meshRef.current?.handleCandidate(event.from_user_id, event.candidate);
      }),
      globalEventBus.on('huddle.mute', (event) => {
        if (event.huddle_id !== active.huddleId || event.user_id === active.selfUserId) return;
        useHuddleStore.getState().setParticipantMuted(event.user_id, event.audio_muted);
      }),
      globalEventBus.on('huddle.camera', (event) => {
        if (event.huddle_id !== active.huddleId || event.user_id === active.selfUserId) return;
        useHuddleStore.getState().setParticipantCamera(event.user_id, event.camera_on);
      }),
      globalEventBus.on('huddle.screenshare', (event) => {
        if (event.huddle_id !== active.huddleId || event.user_id === active.selfUserId) return;
        useHuddleStore.getState().setParticipantSharing(event.user_id, event.sharing);
      }),
      globalEventBus.on('huddle.hand', (event) => {
        if (event.huddle_id !== active.huddleId || event.user_id === active.selfUserId) return;
        useHuddleStore.getState().setParticipantHand(event.user_id, event.raised);
      }),
      globalEventBus.on('huddle.reaction', (event) => {
        if (event.huddle_id !== active.huddleId || event.user_id === active.selfUserId) return;
        const id = crypto.randomUUID();
        useHuddleStore.getState().addReaction({ id, userId: event.user_id, emoji: event.emoji });
        window.setTimeout(() => useHuddleStore.getState().removeReaction(id), 2400);
      }),
    ];

    return () => {
      cancelled = true;
      for (const off of unsubs) off();
      send({ type: 'huddle.leave', huddle_id: active.huddleId });
      meshRef.current?.close();
      meshRef.current = null;
      bgProcessorRef.current?.stop();
      bgProcessorRef.current = null;
      processedTrackRef.current?.stop();
      processedTrackRef.current = null;
      camTrackRef.current?.stop();
      camTrackRef.current = null;
      screenTrackRef.current?.stop();
      screenTrackRef.current = null;
      stopStream(micStreamRef.current);
      micStreamRef.current = null;
      const store = useHuddleStore.getState();
      stopStream(store.localStream);
      store.setLocalStream(null);
      store.setLocalVideoStream(null);
      store.setLocalMuted(false);
      store.setLocalCamera(false);
      store.setLocalSharing(false);
      store.resetParticipants();
    };
  }, [active]);

  const toggleMute = useCallback(() => {
    const a = useHuddleStore.getState().active;
    if (!a) return;
    const next = !useHuddleStore.getState().localMuted;
    micStreamRef.current?.getAudioTracks().forEach((t) => (t.enabled = !next));
    useHuddleStore.getState().setLocalMuted(next);
    wsFor(a).send({ type: 'huddle.mute', huddle_id: a.huddleId, audio_muted: next });
  }, []);

  const updateVideoOutput = useCallback(() => {
    const mesh = meshRef.current;
    if (!mesh) return;
    const store = useHuddleStore.getState();
    let track: MediaStreamTrack | null = null;
    if (store.localSharing) {
      track = screenTrackRef.current;
    } else if (store.localCameraOn) {
      track =
        store.background === 'blur' && processedTrackRef.current
          ? processedTrackRef.current
          : camTrackRef.current;
    }
    mesh.setVideoTrack(track);
    useHuddleStore.getState().setLocalVideoStream(track ? new MediaStream([track]) : null);
  }, []);

  const startBlur = useCallback(async () => {
    if (!camTrackRef.current) return;
    try {
      if (!bgProcessorRef.current) bgProcessorRef.current = new BackgroundProcessor();
      processedTrackRef.current = await bgProcessorRef.current.start(camTrackRef.current);
      updateVideoOutput();
    } catch (err) {
      logger.error('HuddleController', 'startBlur', err);
      useHuddleStore.getState().setBackground('none');
      updateVideoOutput();
    }
  }, [updateVideoOutput]);

  const stopBlur = useCallback(() => {
    bgProcessorRef.current?.stop();
    processedTrackRef.current?.stop();
    processedTrackRef.current = null;
  }, []);

  const toggleCamera = useCallback(async () => {
    const a = useHuddleStore.getState().active;
    if (!a || !meshRef.current) return;
    const store = useHuddleStore.getState();

    if (store.localCameraOn) {
      stopBlur();
      camTrackRef.current?.stop();
      camTrackRef.current = null;
      useHuddleStore.getState().setLocalCamera(false);
      updateVideoOutput();
      wsFor(a).send({ type: 'huddle.camera', huddle_id: a.huddleId, camera_on: false });
      return;
    }

    try {
      const camStream = await acquireCamera(store.devices.cameraId);
      camTrackRef.current = camStream.getVideoTracks()[0];
      useHuddleStore.getState().setLocalCamera(true);
      if (useHuddleStore.getState().background === 'blur' && !useHuddleStore.getState().localSharing) {
        await startBlur();
      } else {
        updateVideoOutput();
      }
      wsFor(a).send({ type: 'huddle.camera', huddle_id: a.huddleId, camera_on: true });
    } catch (err) {
      logger.error('HuddleController', 'toggleCamera', err);
    }
  }, [startBlur, stopBlur, updateVideoOutput]);

  const stopScreenShare = useCallback(() => {
    const a = useHuddleStore.getState().active;
    screenTrackRef.current?.stop();
    screenTrackRef.current = null;
    useHuddleStore.getState().setLocalSharing(false);
    const store = useHuddleStore.getState();
    if (
      store.localCameraOn &&
      store.background === 'blur' &&
      camTrackRef.current &&
      !processedTrackRef.current
    ) {
      void startBlur();
    } else {
      updateVideoOutput();
    }
    if (a) wsFor(a).send({ type: 'huddle.screenshare', huddle_id: a.huddleId, sharing: false });
  }, [startBlur, updateVideoOutput]);

  const toggleScreen = useCallback(async () => {
    const a = useHuddleStore.getState().active;
    if (!a || !meshRef.current) return;
    if (useHuddleStore.getState().localSharing) {
      stopScreenShare();
      return;
    }
    try {
      const screenStream = await acquireScreen();
      screenTrackRef.current = screenStream.getVideoTracks()[0];
      screenTrackRef.current.onended = stopScreenShare;
      useHuddleStore.getState().setLocalSharing(true);
      updateVideoOutput();
      wsFor(a).send({ type: 'huddle.screenshare', huddle_id: a.huddleId, sharing: true });
    } catch (err) {
      logger.error('HuddleController', 'toggleScreen', err);
    }
  }, [stopScreenShare, updateVideoOutput]);

  const toggleBackground = useCallback(async () => {
    const store = useHuddleStore.getState();
    const next = store.background === 'blur' ? 'none' : 'blur';
    store.setBackground(next);
    if (next === 'blur') {
      if (store.localCameraOn && !store.localSharing) await startBlur();
    } else {
      stopBlur();
      updateVideoOutput();
    }
  }, [startBlur, stopBlur, updateVideoOutput]);

  const selectMic = useCallback(async (deviceId: string) => {
    useHuddleStore.getState().setDevice('micId', deviceId);
    try {
      const stream = await acquireLocalAudio(deviceId);
      const track = stream.getAudioTracks()[0];
      track.enabled = !useHuddleStore.getState().localMuted;
      const old = micStreamRef.current;
      micStreamRef.current = stream;
      meshRef.current?.setAudioTrack(track);
      useHuddleStore.getState().setLocalStream(stream);
      stopStream(old);
    } catch (err) {
      logger.error('HuddleController', 'selectMic', err);
    }
  }, []);

  const selectCamera = useCallback(
    async (deviceId: string) => {
      useHuddleStore.getState().setDevice('cameraId', deviceId);
      if (!useHuddleStore.getState().localCameraOn) return;
      try {
        const camStream = await acquireCamera(deviceId);
        stopBlur();
        camTrackRef.current?.stop();
        camTrackRef.current = camStream.getVideoTracks()[0];
        if (useHuddleStore.getState().background === 'blur' && !useHuddleStore.getState().localSharing) {
          await startBlur();
        } else {
          updateVideoOutput();
        }
      } catch (err) {
        logger.error('HuddleController', 'selectCamera', err);
      }
    },
    [startBlur, stopBlur, updateVideoOutput],
  );

  const selectSpeaker = useCallback((deviceId: string) => {
    useHuddleStore.getState().setDevice('speakerId', deviceId);
  }, []);

  const toggleHand = useCallback(() => {
    const a = useHuddleStore.getState().active;
    if (!a) return;
    const raised = !useHuddleStore.getState().localHandRaised;
    useHuddleStore.getState().setLocalHand(raised);
    wsFor(a).send({ type: 'huddle.hand', huddle_id: a.huddleId, raised });
  }, []);

  const sendReaction = useCallback((emoji: string) => {
    const a = useHuddleStore.getState().active;
    if (!a) return;
    wsFor(a).send({ type: 'huddle.reaction', huddle_id: a.huddleId, emoji });
    const id = crypto.randomUUID();
    useHuddleStore.getState().addReaction({ id, userId: a.selfUserId, emoji });
    window.setTimeout(() => useHuddleStore.getState().removeReaction(id), 2400);
  }, []);

  const leave = useCallback(() => useHuddleStore.getState().setActive(null), []);

  const controls: HuddleControls = {
    toggleMute,
    toggleCamera,
    toggleScreen,
    toggleHand,
    toggleBackground,
    sendReaction,
    leave,
    selectMic,
    selectCamera,
    selectSpeaker,
  };

  return (
    <>
      {active && <HuddleWindow controls={controls} />}
      {incomingCalls.map((call) => (
        <IncomingCallRing key={call.huddleId} call={call} />
      ))}
    </>
  );
}
