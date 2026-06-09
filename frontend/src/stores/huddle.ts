import { create } from 'zustand';

export interface HuddleParticipant {
  userId: string;
  stream: MediaStream | null;
  audioMuted: boolean;
  cameraOn: boolean;
  sharing: boolean;
  handRaised: boolean;
}

export interface HuddleReaction {
  id: string;
  userId: string;
  emoji: string;
}

export interface IncomingCall {
  huddleId: string;
  fromUserId: string;
  workspaceId: string;
  instanceUrl: string;
}

export type HuddleScope = { kind: 'dm'; partnerId: string } | { kind: 'channel'; channelId: string };

export interface ActiveHuddle {
  huddleId: string;
  workspaceId: string;
  instanceUrl: string;
  selfUserId: string;
  scope: HuddleScope;
}

export interface SelectedDevices {
  micId: string | null;
  cameraId: string | null;
  speakerId: string | null;
}

interface HuddleState {
  active: ActiveHuddle | null;
  localStream: MediaStream | null;
  localVideoStream: MediaStream | null;
  localMuted: boolean;
  localCameraOn: boolean;
  localSharing: boolean;
  localHandRaised: boolean;
  background: 'none' | 'blur';
  participants: Record<string, HuddleParticipant>;
  speaking: Set<string>;
  pinnedUserId: string | null;
  devices: SelectedDevices;
  reactions: HuddleReaction[];
  incomingCalls: IncomingCall[];

  setActive: (active: ActiveHuddle | null) => void;
  setLocalStream: (stream: MediaStream | null) => void;
  setLocalVideoStream: (stream: MediaStream | null) => void;
  setLocalMuted: (muted: boolean) => void;
  setLocalCamera: (on: boolean) => void;
  setLocalSharing: (sharing: boolean) => void;
  setLocalHand: (raised: boolean) => void;
  setBackground: (mode: 'none' | 'blur') => void;
  upsertParticipant: (userId: string) => void;
  setParticipantStream: (userId: string, stream: MediaStream) => void;
  setParticipantMuted: (userId: string, muted: boolean) => void;
  setParticipantCamera: (userId: string, on: boolean) => void;
  setParticipantSharing: (userId: string, sharing: boolean) => void;
  setParticipantHand: (userId: string, raised: boolean) => void;
  removeParticipant: (userId: string) => void;
  setSpeaking: (userId: string, speaking: boolean) => void;
  setPinned: (userId: string | null) => void;
  setDevice: (kind: keyof SelectedDevices, id: string | null) => void;
  addReaction: (reaction: HuddleReaction) => void;
  removeReaction: (id: string) => void;
  resetParticipants: () => void;
  addIncomingCall: (call: IncomingCall) => void;
  removeIncomingCall: (huddleId: string) => void;
}

const emptyParticipant = (userId: string): HuddleParticipant => ({
  userId,
  stream: null,
  audioMuted: false,
  cameraOn: false,
  sharing: false,
  handRaised: false,
});

export const useHuddleStore = create<HuddleState>((set) => ({
  active: null,
  localStream: null,
  localVideoStream: null,
  localMuted: false,
  localCameraOn: false,
  localSharing: false,
  localHandRaised: false,
  background: 'none',
  participants: {},
  speaking: new Set<string>(),
  pinnedUserId: null,
  devices: { micId: null, cameraId: null, speakerId: null },
  reactions: [],
  incomingCalls: [],

  setActive: (active) => set({ active }),
  setLocalStream: (localStream) => set({ localStream }),
  setLocalVideoStream: (localVideoStream) => set({ localVideoStream }),
  setLocalMuted: (localMuted) => set({ localMuted }),
  setLocalCamera: (localCameraOn) => set({ localCameraOn }),
  setLocalSharing: (localSharing) => set({ localSharing }),
  setLocalHand: (localHandRaised) => set({ localHandRaised }),
  setBackground: (background) => set({ background }),

  upsertParticipant: (userId) =>
    set((s) =>
      s.participants[userId]
        ? s
        : { participants: { ...s.participants, [userId]: emptyParticipant(userId) } },
    ),

  setParticipantStream: (userId, stream) =>
    set((s) => ({
      participants: {
        ...s.participants,
        [userId]: { ...(s.participants[userId] ?? emptyParticipant(userId)), stream },
      },
    })),

  setParticipantMuted: (userId, audioMuted) =>
    set((s) =>
      s.participants[userId]
        ? { participants: { ...s.participants, [userId]: { ...s.participants[userId], audioMuted } } }
        : s,
    ),

  setParticipantCamera: (userId, cameraOn) =>
    set((s) => ({
      participants: {
        ...s.participants,
        [userId]: { ...(s.participants[userId] ?? emptyParticipant(userId)), cameraOn },
      },
    })),

  setParticipantSharing: (userId, sharing) =>
    set((s) => ({
      participants: {
        ...s.participants,
        [userId]: { ...(s.participants[userId] ?? emptyParticipant(userId)), sharing },
      },
    })),

  setParticipantHand: (userId, handRaised) =>
    set((s) => ({
      participants: {
        ...s.participants,
        [userId]: { ...(s.participants[userId] ?? emptyParticipant(userId)), handRaised },
      },
    })),

  removeParticipant: (userId) =>
    set((s) => {
      const next = { ...s.participants };
      delete next[userId];
      const speaking = new Set(s.speaking);
      speaking.delete(userId);
      return {
        participants: next,
        speaking,
        pinnedUserId: s.pinnedUserId === userId ? null : s.pinnedUserId,
      };
    }),

  setSpeaking: (userId, isSpeaking) =>
    set((s) => {
      if (s.speaking.has(userId) === isSpeaking) return s;
      const speaking = new Set(s.speaking);
      if (isSpeaking) speaking.add(userId);
      else speaking.delete(userId);
      return { speaking };
    }),

  setPinned: (pinnedUserId) => set({ pinnedUserId }),

  setDevice: (kind, id) => set((s) => ({ devices: { ...s.devices, [kind]: id } })),

  addReaction: (reaction) => set((s) => ({ reactions: [...s.reactions, reaction] })),

  removeReaction: (id) => set((s) => ({ reactions: s.reactions.filter((r) => r.id !== id) })),

  resetParticipants: () =>
    set({
      participants: {},
      speaking: new Set<string>(),
      pinnedUserId: null,
      reactions: [],
      localHandRaised: false,
    }),

  addIncomingCall: (call) =>
    set((s) =>
      s.incomingCalls.some((c) => c.huddleId === call.huddleId)
        ? s
        : { incomingCalls: [...s.incomingCalls, call] },
    ),

  removeIncomingCall: (huddleId) =>
    set((s) => ({ incomingCalls: s.incomingCalls.filter((c) => c.huddleId !== huddleId) })),
}));
