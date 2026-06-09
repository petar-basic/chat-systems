import { useEffect, useRef, useState } from 'react';
import {
  Mic,
  MicOff,
  Video,
  VideoOff,
  ScreenShare,
  ScreenShareOff,
  PhoneOff,
  Settings,
  Pin,
  Hand,
  Smile,
  Sparkles,
  UserPlus,
  LayoutGrid,
  Maximize2,
  Expand,
  Shrink,
} from 'lucide-react';
import { useHuddleStore, type ActiveHuddle } from '@/stores/huddle';
import { useUserCache } from '@/stores/users';
import { displayNameOf, avatarColorFor } from '@/lib/userHelpers';
import { useWorkspaceMembers } from '@/hooks/queries/useWorkspaces';
import { useInviteToHuddle } from '@/hooks/queries/useHuddle';
import { useSpeaking } from '../hooks/useSpeaking';
import { useMediaDevices } from '../hooks/useMediaDevices';
import type { HuddleControls } from '../HuddleController';

type Layout = 'grid' | 'focus';

const QUICK_REACTIONS = ['👍', '❤️', '😂', '🎉', '👏', '🙌'];

export function HuddleWindow({ controls }: { controls: HuddleControls }) {
  const active = useHuddleStore((s) => s.active);
  const participants = useHuddleStore((s) => s.participants);
  const speaking = useHuddleStore((s) => s.speaking);
  const pinnedUserId = useHuddleStore((s) => s.pinnedUserId);
  const localMuted = useHuddleStore((s) => s.localMuted);
  const localCameraOn = useHuddleStore((s) => s.localCameraOn);
  const localSharing = useHuddleStore((s) => s.localSharing);
  const localHandRaised = useHuddleStore((s) => s.localHandRaised);
  const background = useHuddleStore((s) => s.background);
  const localVideoStream = useHuddleStore((s) => s.localVideoStream);
  const localStream = useHuddleStore((s) => s.localStream);
  const speakerId = useHuddleStore((s) => s.devices.speakerId);
  const reactions = useHuddleStore((s) => s.reactions);
  const [layout, setLayout] = useState<Layout>('grid');
  const [expanded, setExpanded] = useState(false);

  if (!active) return null;

  const self = active.selfUserId;
  const remotes = Object.values(participants).filter((p) => p.userId !== self);

  const tiles = [
    {
      userId: self,
      stream: localVideoStream,
      audioStream: null as MediaStream | null,
      muted: localMuted,
      cameraOn: localCameraOn,
      sharing: localSharing,
      handRaised: localHandRaised,
      isSelf: true,
    },
    ...remotes.map((p) => ({
      userId: p.userId,
      stream: p.stream,
      audioStream: p.stream,
      muted: p.audioMuted,
      cameraOn: p.cameraOn,
      sharing: p.sharing,
      handRaised: p.handRaised,
      isSelf: false,
    })),
  ];

  const activeSpeaker = [...speaking].find((id) => id !== self) ?? null;
  const focusId = pinnedUserId ?? activeSpeaker ?? remotes[0]?.userId ?? self;
  const focusTile = tiles.find((t) => t.userId === focusId) ?? tiles[0];
  const stripTiles = tiles.filter((t) => t.userId !== focusTile.userId);

  return (
    <div
      role="dialog"
      aria-label="Huddle"
      className={
        expanded
          ? 'fixed inset-0 z-60 bg-slate-900 p-4 flex flex-col gap-3'
          : 'fixed bottom-4 right-4 z-60 w-120 max-w-[calc(100vw-2rem)] bg-slate-800 border border-slate-700 rounded-2xl shadow-2xl p-3 flex flex-col gap-3'
      }
    >
      {tiles.map((t) => (
        <SpeakingDetector key={`spk-${t.userId}`} userId={t.userId} stream={t.audioStream ?? t.stream} />
      ))}

      <div className="flex items-center justify-between">
        <span className="text-sm font-semibold text-white">
          Huddle <span className="text-slate-400 font-normal">· {tiles.length}</span>
        </span>
        <div className="flex items-center gap-1">
          <button
            onClick={() => setLayout((l) => (l === 'grid' ? 'focus' : 'grid'))}
            aria-label={layout === 'grid' ? 'Focus view' : 'Grid view'}
            title={layout === 'grid' ? 'Focus view' : 'Grid view'}
            className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-700/50 transition"
          >
            {layout === 'grid' ? <Maximize2 className="w-4 h-4" /> : <LayoutGrid className="w-4 h-4" />}
          </button>
          <button
            onClick={() => setExpanded((v) => !v)}
            aria-label={expanded ? 'Collapse huddle' : 'Expand huddle'}
            title={expanded ? 'Collapse' : 'Full screen'}
            className="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-700/50 transition"
          >
            {expanded ? <Shrink className="w-4 h-4" /> : <Expand className="w-4 h-4" />}
          </button>
        </div>
      </div>

      <div className={`relative ${expanded ? 'flex-1 min-h-0 overflow-y-auto' : ''}`}>
        {layout === 'focus' ? (
          <div className="flex flex-col gap-2">
            <VideoTile
              {...focusTile}
              speaking={speaking.has(focusTile.userId)}
              pinned={pinnedUserId === focusTile.userId}
              speakerId={speakerId}
              large
            />
            {stripTiles.length > 0 && (
              <div className="flex gap-2 overflow-x-auto">
                {stripTiles.map((t) => (
                  <div key={t.userId} className="w-28 shrink-0">
                    <VideoTile
                      {...t}
                      speaking={speaking.has(t.userId)}
                      pinned={pinnedUserId === t.userId}
                      speakerId={speakerId}
                    />
                  </div>
                ))}
              </div>
            )}
          </div>
        ) : (
          <div
            className={`grid gap-2 ${expanded ? 'grid-cols-2 md:grid-cols-3 lg:grid-cols-4 content-start' : 'grid-cols-2'}`}
          >
            {tiles.map((t) => (
              <VideoTile
                key={t.userId}
                {...t}
                speaking={speaking.has(t.userId)}
                pinned={pinnedUserId === t.userId}
                speakerId={speakerId}
              />
            ))}
          </div>
        )}

        {reactions.length > 0 && (
          <div className="pointer-events-none absolute inset-x-0 bottom-2 flex justify-center gap-2">
            {reactions.map((r) => (
              <span key={r.id} className="huddle-float text-3xl drop-shadow-lg">
                {r.emoji}
              </span>
            ))}
          </div>
        )}
      </div>

      <div className="flex items-center justify-center gap-1.5 pt-1">
        <ControlButton
          active={!localMuted}
          onClick={controls.toggleMute}
          on={<Mic className="w-4 h-4" />}
          off={<MicOff className="w-4 h-4" />}
          label={localMuted ? 'Unmute' : 'Mute'}
          danger={localMuted}
        />
        <ControlButton
          active={localCameraOn}
          onClick={() => void controls.toggleCamera()}
          on={<Video className="w-4 h-4" />}
          off={<VideoOff className="w-4 h-4" />}
          label={localCameraOn ? 'Turn camera off' : 'Turn camera on'}
        />
        <ControlButton
          active={localSharing}
          onClick={() => void controls.toggleScreen()}
          on={<ScreenShare className="w-4 h-4" />}
          off={<ScreenShareOff className="w-4 h-4" />}
          label={localSharing ? 'Stop sharing' : 'Share screen'}
          highlight={localSharing}
        />
        <ControlButton
          active={localHandRaised}
          onClick={controls.toggleHand}
          on={<Hand className="w-4 h-4" />}
          off={<Hand className="w-4 h-4" />}
          label={localHandRaised ? 'Lower hand' : 'Raise hand'}
          highlight={localHandRaised}
        />
        <ControlButton
          active={background === 'blur'}
          onClick={() => void controls.toggleBackground()}
          on={<Sparkles className="w-4 h-4" />}
          off={<Sparkles className="w-4 h-4" />}
          label={background === 'blur' ? 'Background blur on' : 'Blur background'}
          highlight={background === 'blur'}
        />
        <ReactionPicker onPick={controls.sendReaction} />
        <InvitePicker active={active} />
        <DevicePicker controls={controls} />
        <button
          onClick={controls.leave}
          aria-label="Leave huddle"
          className="p-2.5 rounded-full bg-red-600 text-white hover:bg-red-500 transition"
        >
          <PhoneOff className="w-4 h-4" />
        </button>
      </div>

      {!localStream && <p className="text-xs text-slate-400 text-center">Connecting…</p>}
    </div>
  );
}

interface TileProps {
  userId: string;
  stream: MediaStream | null;
  muted: boolean;
  cameraOn: boolean;
  sharing: boolean;
  handRaised: boolean;
  isSelf: boolean;
  speaking: boolean;
  pinned: boolean;
  speakerId: string | null;
  large?: boolean;
}

type SinkVideo = HTMLVideoElement & { setSinkId?: (id: string) => Promise<void> };

function VideoTile({
  userId,
  stream,
  muted,
  cameraOn,
  sharing,
  handRaised,
  isSelf,
  speaking,
  pinned,
  speakerId,
  large,
}: TileProps) {
  const { getUser } = useUserCache();
  const name = displayNameOf(getUser(userId)?.display_name);
  const videoRef = useRef<HTMLVideoElement>(null);
  const hasVideo = (cameraOn || sharing) && !!stream;

  useEffect(() => {
    const el = videoRef.current;
    if (el && el.srcObject !== stream) el.srcObject = stream;
  }, [stream]);

  useEffect(() => {
    const el = videoRef.current as SinkVideo | null;
    if (el?.setSinkId && speakerId && !isSelf) {
      void el.setSinkId(speakerId).catch(() => undefined);
    }
  }, [speakerId, isSelf]);

  return (
    <div
      className={`relative ${large ? 'aspect-video' : 'aspect-[4/3]'} rounded-lg overflow-hidden bg-slate-900 ring-2 transition ${
        speaking && !muted ? 'ring-green-500/70' : 'ring-transparent'
      }`}
    >
      <video
        ref={videoRef}
        autoPlay
        playsInline
        muted={isSelf}
        className={`w-full h-full object-cover ${hasVideo ? '' : 'hidden'} ${isSelf && !sharing ? 'scale-x-[-1]' : ''}`}
      />
      {!hasVideo && (
        <div className="absolute inset-0 flex items-center justify-center">
          <div
            className={`${large ? 'w-16 h-16 text-2xl' : 'w-10 h-10 text-sm'} rounded-full ${avatarColorFor(userId)} flex items-center justify-center font-bold`}
          >
            {name.charAt(0).toUpperCase()}
          </div>
        </div>
      )}

      {handRaised && (
        <div className="absolute top-1 left-1 text-lg" aria-label="Hand raised" title="Hand raised">
          ✋
        </div>
      )}

      <div className="absolute bottom-1 left-1 right-1 flex items-center gap-1">
        <span className="px-1.5 py-0.5 rounded bg-black/50 text-[11px] text-white truncate max-w-full">
          {name}
          {isSelf && ' (you)'}
        </span>
        {sharing && (
          <span className="px-1 py-0.5 rounded bg-purple-600/80 text-[10px] text-white">Sharing</span>
        )}
        {muted && <MicOff className="w-3 h-3 text-red-400 shrink-0" />}
      </div>

      {!isSelf && (
        <button
          onClick={() => useHuddleStore.getState().setPinned(pinned ? null : userId)}
          aria-label={pinned ? 'Unpin' : 'Pin'}
          title={pinned ? 'Unpin' : 'Pin'}
          className={`absolute top-1 right-1 p-1 rounded-md transition ${
            pinned ? 'bg-purple-600 text-white' : 'bg-black/40 text-slate-200'
          }`}
        >
          <Pin className="w-3 h-3" />
        </button>
      )}
    </div>
  );
}

function SpeakingDetector({ userId, stream }: { userId: string; stream: MediaStream | null }) {
  const speaking = useSpeaking(stream);
  useEffect(() => {
    useHuddleStore.getState().setSpeaking(userId, speaking);
  }, [userId, speaking]);
  return null;
}

interface ControlButtonProps {
  active: boolean;
  onClick: () => void;
  on: React.ReactNode;
  off: React.ReactNode;
  label: string;
  danger?: boolean;
  highlight?: boolean;
}

function ControlButton({ active, onClick, on, off, label, danger, highlight }: ControlButtonProps) {
  const tone = danger
    ? 'bg-red-600/20 text-red-400 hover:bg-red-600/30'
    : highlight
      ? 'bg-purple-600/30 text-purple-300 hover:bg-purple-600/40'
      : 'bg-slate-700 text-white hover:bg-slate-600';
  return (
    <button
      onClick={onClick}
      aria-label={label}
      title={label}
      aria-pressed={active}
      className={`p-2.5 rounded-full transition ${tone}`}
    >
      {active ? on : off}
    </button>
  );
}

function ReactionPicker({ onPick }: { onPick: (emoji: string) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        aria-label="Send reaction"
        title="Send reaction"
        aria-expanded={open}
        className="p-2.5 rounded-full bg-slate-700 text-white hover:bg-slate-600 transition"
      >
        <Smile className="w-4 h-4" />
      </button>
      {open && (
        <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 flex gap-1 bg-slate-900 border border-slate-700 rounded-xl p-1.5 shadow-2xl">
          {QUICK_REACTIONS.map((emoji) => (
            <button
              key={emoji}
              onClick={() => {
                onPick(emoji);
                setOpen(false);
              }}
              className="text-xl px-1.5 py-0.5 rounded-lg hover:bg-slate-700 transition"
            >
              {emoji}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function InvitePicker({ active }: { active: ActiveHuddle }) {
  const [open, setOpen] = useState(false);
  const { data: members } = useWorkspaceMembers(open ? active.workspaceId : null, active.instanceUrl);
  const invite = useInviteToHuddle(active.workspaceId, active.huddleId, active.instanceUrl);
  const participants = useHuddleStore((s) => s.participants);

  const inHuddle = new Set([active.selfUserId, ...Object.keys(participants)]);
  const candidates = (members ?? []).filter((m) => !inHuddle.has(m.user_id));

  return (
    <div className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        aria-label="Invite to huddle"
        title="Invite to huddle"
        aria-expanded={open}
        className="p-2.5 rounded-full bg-slate-700 text-white hover:bg-slate-600 transition"
      >
        <UserPlus className="w-4 h-4" />
      </button>
      {open && (
        <div className="absolute bottom-full right-0 mb-2 w-56 max-h-64 overflow-y-auto bg-slate-900 border border-slate-700 rounded-xl p-2 shadow-2xl flex flex-col gap-0.5">
          {candidates.length === 0 && <p className="text-xs text-slate-400 px-2 py-1.5">No one to invite</p>}
          {candidates.map((m) => (
            <button
              key={m.user_id}
              onClick={() => {
                invite.mutate([m.user_id]);
                setOpen(false);
              }}
              className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-left text-sm text-slate-200 hover:bg-slate-700 transition"
            >
              <span
                className={`w-6 h-6 rounded-full ${avatarColorFor(m.user_id)} flex items-center justify-center text-xs font-bold shrink-0`}
              >
                {displayNameOf(m.display_name).charAt(0).toUpperCase()}
              </span>
              <span className="truncate">{displayNameOf(m.display_name)}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function DevicePicker({ controls }: { controls: HuddleControls }) {
  const [open, setOpen] = useState(false);
  const devices = useMediaDevices(open);
  const selected = useHuddleStore((s) => s.devices);

  return (
    <div className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        aria-label="Devices"
        title="Devices"
        aria-expanded={open}
        className="p-2.5 rounded-full bg-slate-700 text-white hover:bg-slate-600 transition"
      >
        <Settings className="w-4 h-4" />
      </button>
      {open && (
        <div className="absolute bottom-full right-0 mb-2 w-60 bg-slate-900 border border-slate-700 rounded-xl p-3 shadow-2xl flex flex-col gap-2">
          <DeviceSelect
            label="Microphone"
            options={devices.mics}
            value={selected.micId}
            onChange={controls.selectMic}
          />
          <DeviceSelect
            label="Camera"
            options={devices.cameras}
            value={selected.cameraId}
            onChange={controls.selectCamera}
          />
          {devices.speakers.length > 0 && (
            <DeviceSelect
              label="Speaker"
              options={devices.speakers}
              value={selected.speakerId}
              onChange={controls.selectSpeaker}
            />
          )}
        </div>
      )}
    </div>
  );
}

interface DeviceSelectProps {
  label: string;
  options: MediaDeviceInfo[];
  value: string | null;
  onChange: (deviceId: string) => void;
}

function DeviceSelect({ label, options, value, onChange }: DeviceSelectProps) {
  return (
    <label className="flex flex-col gap-1 text-xs text-slate-300">
      <span>{label}</span>
      <select
        value={value ?? options[0]?.deviceId ?? ''}
        onChange={(e) => onChange(e.target.value)}
        className="bg-slate-800 border border-slate-700 rounded-lg px-2 py-1.5 text-white text-xs"
      >
        {options.length === 0 && <option value="">No devices</option>}
        {options.map((d, i) => (
          <option key={d.deviceId} value={d.deviceId}>
            {d.label || `${label} ${i + 1}`}
          </option>
        ))}
      </select>
    </label>
  );
}
