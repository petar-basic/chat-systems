import { useState } from 'react';
import {
  Mic,
  MicOff,
  Video,
  VideoOff,
  ScreenShare,
  ScreenShareOff,
  PhoneOff,
  Settings,
  Hand,
  Smile,
  Sparkles,
  UserPlus,
} from 'lucide-react';
import { useHuddleStore, type ActiveHuddle } from '@/stores/huddle';
import { displayNameOf } from '@/lib/userHelpers';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { useWorkspaceMembers } from '@/features/workspace/hooks/useWorkspaces';
import { useInviteToHuddle } from '@/features/huddle/hooks/useHuddle';
import { useMediaDevices } from '../hooks/useMediaDevices';
import type { HuddleControls } from '../HuddleController';
import { Select } from '@/shared/components/Form/Select';

const QUICK_REACTIONS = ['👍', '❤️', '😂', '🎉', '👏', '🙌'];

export function HuddleControlBar({ controls, active }: { controls: HuddleControls; active: ActiveHuddle }) {
  const localMuted = useHuddleStore((s) => s.localMuted);
  const localCameraOn = useHuddleStore((s) => s.localCameraOn);
  const localSharing = useHuddleStore((s) => s.localSharing);
  const localHandRaised = useHuddleStore((s) => s.localHandRaised);
  const background = useHuddleStore((s) => s.background);

  return (
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
  );
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
    ? 'bg-red-600/20 text-danger hover:bg-red-600/30'
    : highlight
      ? 'bg-purple-600/30 text-accent-soft hover:bg-purple-600/40'
      : 'bg-raised text-fg hover:bg-elevated';
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
        className="p-2.5 rounded-full bg-raised text-fg hover:bg-elevated transition"
      >
        <Smile className="w-4 h-4" />
      </button>
      {open && (
        <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 flex gap-1 bg-app border border-line rounded-xl p-1.5 shadow-2xl">
          {QUICK_REACTIONS.map((emoji) => (
            <button
              key={emoji}
              onClick={() => {
                onPick(emoji);
                setOpen(false);
              }}
              className="text-xl px-1.5 py-0.5 rounded-lg hover:bg-raised transition"
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
        className="p-2.5 rounded-full bg-raised text-fg hover:bg-elevated transition"
      >
        <UserPlus className="w-4 h-4" />
      </button>
      {open && (
        <div className="absolute bottom-full right-0 mb-2 w-56 max-h-64 overflow-y-auto bg-app border border-line rounded-xl p-2 shadow-2xl flex flex-col gap-0.5">
          {candidates.length === 0 && <p className="text-xs text-muted px-2 py-1.5">No one to invite</p>}
          {candidates.map((m) => (
            <button
              key={m.user_id}
              onClick={() => {
                invite.mutate([m.user_id]);
                setOpen(false);
              }}
              className="flex items-center gap-2 px-2 py-1.5 rounded-lg text-left text-sm text-fg-soft hover:bg-raised transition"
            >
              <Avatar
                userId={m.user_id}
                name={displayNameOf(m.display_name)}
                avatarUrl={m.avatar_url}
                size="xs"
              />
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
        className="p-2.5 rounded-full bg-raised text-fg hover:bg-elevated transition"
      >
        <Settings className="w-4 h-4" />
      </button>
      {open && (
        <div className="absolute bottom-full right-0 mb-2 w-60 bg-app border border-line rounded-xl p-3 shadow-2xl flex flex-col gap-2">
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
    <Select
      label={label}
      compact
      value={value ?? options[0]?.deviceId ?? ''}
      onChange={(e) => onChange(e.target.value)}
    >
      {options.length === 0 && <option value="">No devices</option>}
      {options.map((d, i) => (
        <option key={d.deviceId} value={d.deviceId}>
          {d.label || `${label} ${i + 1}`}
        </option>
      ))}
    </Select>
  );
}
