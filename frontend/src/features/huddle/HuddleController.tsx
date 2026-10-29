import { useCallback, useEffect, useMemo, useRef } from 'react';
import { globalEventBus } from '@/lib/globalEventBus';
import { instanceManager } from '@/lib/instances';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { useWorkspaceStore } from '@/stores/workspace';
import { useHuddleStore, type ActiveHuddle } from '@/stores/huddle';
import { logger } from '@/lib/logger';
import type { HuddleTransport, TrackKind, TransportHandlers } from './lib/HuddleTransport';
import { MeshManager } from './lib/MeshManager';
import { openSfu } from './lib/openSfu';
import { Promotion, type LocalMedia } from './lib/Promotion';
import { acquireCamera, acquireLocalAudio, acquireScreen, micErrorLabel, stopStream } from './lib/media';
import { toast } from '@/shared/components/Toast';
import { ErrorLabels } from '@/shared/constants';
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

const joinMessage = (active: ActiveHuddle) => ({ type: 'huddle.join', huddle_id: active.huddleId });

export function HuddleController() {
  const active = useHuddleStore((s) => s.active);
  const incomingCalls = useHuddleStore((s) => s.incomingCalls);

  const transportRef = useRef<HuddleTransport | null>(null);
  const micStreamRef = useRef<MediaStream | null>(null);
  const camTrackRef = useRef<MediaStreamTrack | null>(null);
  const screenTrackRef = useRef<MediaStreamTrack | null>(null);
  const processedTrackRef = useRef<MediaStreamTrack | null>(null);
  const bgProcessorRef = useRef<BackgroundProcessor | null>(null);
  const joinedRef = useRef(false);
  const rejoiningRef = useRef(false);
  const rejoinRef = useRef<(() => void) | null>(null);

  const handlers = useMemo<TransportHandlers>(
    () => ({
      onTrack: (peerId: string, remote: MediaStream | null, kind: TrackKind) =>
        useHuddleStore.getState().setParticipantStream(peerId, kind, remote),
      onSpeakers: (userIds: string[]) => useHuddleStore.getState().setSpeakers(userIds),
    }),
    [],
  );

  const localMedia = useCallback((): LocalMedia => {
    const store = useHuddleStore.getState();
    return {
      audio: micStreamRef.current?.getAudioTracks()[0] ?? null,
      camera: store.localCameraOn
        ? store.background === 'blur' && processedTrackRef.current
          ? processedTrackRef.current
          : camTrackRef.current
        : null,
      screen: store.localSharing ? screenTrackRef.current : null,
    };
  }, []);

  const announceLocalState = useCallback(() => {
    const a = useHuddleStore.getState().active;
    if (!a) return;
    const ws = wsFor(a);
    const st = useHuddleStore.getState();
    if (st.localMuted) ws.send({ type: 'huddle.mute', huddle_id: a.huddleId, audio_muted: true });
    if (st.localCameraOn) ws.send({ type: 'huddle.camera', huddle_id: a.huddleId, camera_on: true });
    if (st.localSharing) ws.send({ type: 'huddle.screenshare', huddle_id: a.huddleId, sharing: true });
    if (st.localHandRaised) ws.send({ type: 'huddle.hand', huddle_id: a.huddleId, raised: true });
  }, []);

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
    let iceServers: RTCIceServer[] = [];
    let promoting = false;

    const promote = async () => {
      const mesh = transportRef.current;
      if (promoting || !mesh) return;
      promoting = true;
      const store = useHuddleStore.getState();
      const peers = Object.keys(store.participants).filter((id) => id !== active.selfUserId);
      const promotion = new Promotion(mesh, peers, handlers, store.localMuted, localMedia());
      transportRef.current = promotion;
      try {
        await promotion.run((sfuHandlers) => openSfu(active, iceServers, sfuHandlers));
      } catch (err) {
        logger.error('HuddleController', 'sfu', err);
        if (transportRef.current === promotion) transportRef.current = mesh;
        if (!cancelled) toast.error(ErrorLabels.HuddleSfuUnreachable);
        return;
      }
      if (!cancelled) useHuddleStore.getState().setTransport('sfu');
    };

    const start = async () => {
      try {
        const res = await getApiForInstance(active.instanceUrl).typed((c) =>
          c.GET('/workspaces/{ws_id}/ice-servers', { params: { path: { ws_id: active.workspaceId } } }),
        );
        iceServers = res.ice_servers.map((server) => ({
          urls: server.urls,
          username: server.username ?? undefined,
          credential: server.credential ?? undefined,
        }));
      } catch (err) {
        // Joining anyway leaves only host candidates, which connects on a LAN
        // and nowhere else — a call that fails silently is worse than one that
        // says it could not start.
        logger.error('HuddleController', 'ice-servers', err);
        toast.error(ErrorLabels.HuddleSetupFailed);
        useHuddleStore.getState().setActive(null);
        return;
      }
      if (cancelled) return;

      if (iceServers.length === 0) {
        logger.warn('HuddleController', 'ice-servers', 'no ICE servers configured');
      }

      let stream: MediaStream;
      try {
        stream = await acquireLocalAudio(useHuddleStore.getState().devices.micId);
      } catch (err) {
        logger.error('HuddleController', 'getUserMedia', err);
        toast.error(micErrorLabel(err));
        useHuddleStore.getState().setActive(null);
        return;
      }
      if (cancelled) {
        stopStream(stream);
        return;
      }

      micStreamRef.current = stream;
      useHuddleStore.getState().setLocalStream(stream);

      const mesh = new MeshManager(active.huddleId, active.selfUserId, iceServers, send, handlers);
      mesh.setMuted(useHuddleStore.getState().localMuted);
      mesh.setTrack('audio', stream.getAudioTracks()[0] ?? null);
      transportRef.current = mesh;

      send(joinMessage(active));
      joinedRef.current = true;
    };

    void start();

    const offReconnect = ws.addReconnectListener(() => rejoinRef.current?.());

    const unsubs = [
      globalEventBus.on('huddle.full', (event) => {
        if (event.huddle_id !== active.huddleId) return;
        joinedRef.current = false;
        toast.error(ErrorLabels.huddleFull(event.limit));
        useHuddleStore.getState().setActive(null);
      }),
      globalEventBus.on('huddle.transport', (event) => {
        if (event.huddle_id === active.huddleId && event.transport === 'sfu') void promote();
      }),
      globalEventBus.on('huddle.members', (event) => {
        if (event.huddle_id !== active.huddleId) return;
        if (event.transport === 'sfu') void promote();
        for (const uid of event.user_ids) {
          if (uid === active.selfUserId) continue;
          useHuddleStore.getState().upsertParticipant(uid);
          transportRef.current?.addPeer(uid);
        }
      }),
      globalEventBus.on('huddle.member_joined', (event) => {
        if (event.huddle_id !== active.huddleId || event.user_id === active.selfUserId) return;
        useHuddleStore.getState().upsertParticipant(event.user_id);
        transportRef.current?.addPeer(event.user_id);
        announceLocalState();
      }),
      globalEventBus.on('huddle.member_left', (event) => {
        if (event.huddle_id !== active.huddleId) return;
        transportRef.current?.removePeer(event.user_id);
        useHuddleStore.getState().removeParticipant(event.user_id);
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
      const joined = joinedRef.current;
      joinedRef.current = false;
      offReconnect();
      for (const off of unsubs) off();
      if (joined) send({ type: 'huddle.leave', huddle_id: active.huddleId });
      transportRef.current?.close();
      transportRef.current = null;
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
      store.setLocalScreenStream(null);
      store.setLocalMuted(false);
      store.setLocalCamera(false);
      store.setLocalSharing(false);
      store.resetParticipants();
      store.setTransport('mesh');
    };
  }, [active, handlers, announceLocalState, localMedia]);

  const toggleMute = useCallback(() => {
    const a = useHuddleStore.getState().active;
    if (!a) return;
    const next = !useHuddleStore.getState().localMuted;
    transportRef.current?.setMuted(next);
    useHuddleStore.getState().setLocalMuted(next);
    wsFor(a).send({ type: 'huddle.mute', huddle_id: a.huddleId, audio_muted: next });
  }, []);

  // Camera and screen ride their own transceivers, so sharing never costs the
  // person their face: both go out at once, the way Slack does it.
  const updateVideoOutput = useCallback(() => {
    const transport = transportRef.current;
    if (!transport) return;
    const store = useHuddleStore.getState();
    const { camera, screen } = localMedia();

    transport.setTrack('camera', camera);
    transport.setTrack('screen', screen);
    store.setLocalVideoStream(camera ? new MediaStream([camera]) : null);
    store.setLocalScreenStream(screen ? new MediaStream([screen]) : null);
  }, [localMedia]);

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

  const rejoin = useCallback(() => {
    const a = useHuddleStore.getState().active;
    if (!a || !joinedRef.current || rejoiningRef.current) return;
    rejoiningRef.current = true;
    try {
      useHuddleStore.getState().resetParticipants();
      transportRef.current?.restart();
      wsFor(a).send(joinMessage(a));
      announceLocalState();
    } finally {
      rejoiningRef.current = false;
    }
  }, [announceLocalState]);

  useEffect(() => {
    rejoinRef.current = rejoin;
  }, [rejoin]);

  const toggleCamera = useCallback(async () => {
    const a = useHuddleStore.getState().active;
    if (!a || !transportRef.current) return;
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
      if (useHuddleStore.getState().background === 'blur') {
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
    updateVideoOutput();
    if (a) wsFor(a).send({ type: 'huddle.screenshare', huddle_id: a.huddleId, sharing: false });
  }, [updateVideoOutput]);

  const toggleScreen = useCallback(async () => {
    const a = useHuddleStore.getState().active;
    if (!a || !transportRef.current) return;
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
      if (store.localCameraOn) await startBlur();
    } else {
      stopBlur();
      updateVideoOutput();
    }
  }, [startBlur, stopBlur, updateVideoOutput]);

  const selectMic = useCallback(async (deviceId: string) => {
    useHuddleStore.getState().setDevice('micId', deviceId);
    try {
      const stream = await acquireLocalAudio(deviceId);
      const old = micStreamRef.current;
      micStreamRef.current = stream;
      transportRef.current?.setTrack('audio', stream.getAudioTracks()[0] ?? null);
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
        if (useHuddleStore.getState().background === 'blur') {
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
