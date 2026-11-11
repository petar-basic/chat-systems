import { useState } from 'react';
import { useTranslation } from 'react-i18next';
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
import { useWorkspaceMembers } from '@/features/workspace';
import { useInviteToHuddle } from '@/features/huddle/hooks/useHuddle';
import { useMediaDevices } from '../hooks/useMediaDevices';
import type { HuddleControls } from '../HuddleController';
import { Select } from '@/shared/components/Form/Select';

const QUICK_REACTIONS = ['👍', '❤️', '😂', '🎉', '👏', '🙌'];

export function HuddleControlBar({ controls, active }: { controls: HuddleControls; active: ActiveHuddle }) {
  const { t } = useTranslation('huddle');
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
        label={localMuted ? t('controls.unmute') : t('controls.mute')}
        danger={localMuted}
      />
      <ControlButton
        active={localCameraOn}
        onClick={() => void controls.toggleCamera()}
        on={<Video className="w-4 h-4" />}
        off={<VideoOff className="w-4 h-4" />}
        label={localCameraOn ? t('controls.cameraOff') : t('controls.cameraOn')}
      />
      <ControlButton
        active={localSharing}
        onClick={() => void controls.toggleScreen()}
        on={<ScreenShare className="w-4 h-4" />}
        off={<ScreenShareOff className="w-4 h-4" />}
        label={localSharing ? t('controls.stopSharing') : t('controls.share')}
        highlight={localSharing}
      />
      <ControlButton
        active={localHandRaised}
        onClick={controls.toggleHand}
        on={<Hand className="w-4 h-4" />}
        off={<Hand className="w-4 h-4" />}
        label={localHandRaised ? t('controls.lowerHand') : t('controls.raiseHand')}
        highlight={localHandRaised}
      />
      <ControlButton
        active={background === 'blur'}
        onClick={() => void controls.toggleBackground()}
        on={<Sparkles className="w-4 h-4" />}
        off={<Sparkles className="w-4 h-4" />}
        label={background === 'blur' ? t('controls.blurOn') : t('controls.blur')}
        highlight={background === 'blur'}
      />
      <ReactionPicker onPick={controls.sendReaction} />
      <InvitePicker active={active} />
      <DevicePicker controls={controls} />
      <button
        onClick={controls.leave}
        aria-label={t('controls.leave')}
        data-tooltip={t('controls.leave')}
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
      data-tooltip={label}
      aria-pressed={active}
      className={`p-2.5 rounded-full transition ${tone}`}
    >
      {active ? on : off}
    </button>
  );
}

function ReactionPicker({ onPick }: { onPick: (emoji: string) => void }) {
  const { t } = useTranslation('huddle');
  const [open, setOpen] = useState(false);
  return (
    <div className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        aria-label={t('controls.react')}
        data-tooltip={t('controls.react')}
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
  const { t } = useTranslation('huddle');
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
        aria-label={t('controls.invite')}
        data-tooltip={t('controls.invite')}
        aria-expanded={open}
        className="p-2.5 rounded-full bg-raised text-fg hover:bg-elevated transition"
      >
        <UserPlus className="w-4 h-4" />
      </button>
      {open && (
        <div className="absolute bottom-full right-0 mb-2 w-56 max-h-64 overflow-y-auto bg-app border border-line rounded-xl p-2 shadow-2xl flex flex-col gap-0.5">
          {candidates.length === 0 && (
            <p className="text-xs text-muted px-2 py-1.5">{t('controls.noOneToInvite')}</p>
          )}
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
  const { t } = useTranslation('huddle');
  const [open, setOpen] = useState(false);
  const devices = useMediaDevices(open);
  const selected = useHuddleStore((s) => s.devices);

  return (
    <div className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        aria-label={t('controls.devices')}
        data-tooltip={t('controls.devices')}
        aria-expanded={open}
        className="p-2.5 rounded-full bg-raised text-fg hover:bg-elevated transition"
      >
        <Settings className="w-4 h-4" />
      </button>
      {open && (
        <div className="absolute bottom-full right-0 mb-2 w-60 bg-app border border-line rounded-xl p-3 shadow-2xl flex flex-col gap-2">
          <DeviceSelect
            label={t('controls.mic')}
            options={devices.mics}
            value={selected.micId}
            onChange={controls.selectMic}
          />
          <DeviceSelect
            label={t('controls.camera')}
            options={devices.cameras}
            value={selected.cameraId}
            onChange={controls.selectCamera}
          />
          {devices.speakers.length > 0 && (
            <DeviceSelect
              label={t('controls.speaker')}
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
  const { t } = useTranslation('huddle');
  return (
    <Select
      label={label}
      compact
      value={value ?? options[0]?.deviceId ?? ''}
      onChange={(e) => onChange(e.target.value)}
    >
      {options.length === 0 && <option value="">{t('controls.noDevices')}</option>}
      {options.map((d, i) => (
        <option key={d.deviceId} value={d.deviceId}>
          {d.label || `${label} ${i + 1}`}
        </option>
      ))}
    </Select>
  );
}
