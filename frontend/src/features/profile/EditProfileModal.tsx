import { useState, type FormEvent } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { Camera } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { TextInput } from '@/shared/components/Form/TextInput';
import { TextArea } from '@/shared/components/Form/TextArea';
import { FilePicker } from '@/shared/components/Form/FilePicker';
import { buttonStyles } from '@/shared/components/Button/buttonStyles';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { useCurrentUser } from '@/features/instance/hooks/useAuth';
import { useMyStatus, useSetStatus } from '@/features/profile/hooks/useStatus';
import { useInstanceStore } from '@/stores/instances';
import { useWorkspaceStore } from '@/stores/workspace';
import { displayNameOf } from '@/lib/userHelpers';
import { toUserMessage } from '@/lib/errors';
import { StatusFields } from './StatusFields';
import { sameStatus, type StatusDraft } from './statusDraft';

const EMPTY_STATUS: StatusDraft = { emoji: '', text: '', minutes: null };

interface Props {
  onClose: () => void;
}

export function EditProfileModal({ onClose }: Props) {
  const queryClient = useQueryClient();
  const instanceUrl = useInstanceStore((s) => s.activeInstanceUrl) ?? undefined;
  const updateInstanceUser = useInstanceStore((s) => s.updateInstanceUser);
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const { data: user } = useCurrentUser();
  const { data: me, isLoading } = useMyStatus(instanceUrl);
  const { set: setStatus, clear: clearStatus } = useSetStatus(instanceUrl, workspace?.id);

  const [loaded, setLoaded] = useState(false);
  const [displayName, setDisplayName] = useState('');
  const [bio, setBio] = useState('');
  const [avatarUrl, setAvatarUrl] = useState('');
  const [status, setStatusDraft] = useState<StatusDraft>(EMPTY_STATUS);
  const [savedStatus, setSavedStatus] = useState<StatusDraft>(EMPTY_STATUS);
  const [uploading, setUploading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!loaded && me) {
    const current = { emoji: me.status_emoji ?? '', text: me.status_text ?? '', minutes: null };
    setLoaded(true);
    setDisplayName(me.display_name ?? '');
    setBio(me.bio ?? '');
    setAvatarUrl(me.avatar_url ?? '');
    setStatusDraft(current);
    setSavedStatus(current);
  }

  const statusChanged = !sameStatus(status, savedStatus);
  const api = getApiForInstance(instanceUrl);

  const saveStatus = async () => {
    if (!statusChanged) return;
    const emoji = status.emoji.trim();
    const text = status.text.trim();
    if (!emoji && !text) {
      await clearStatus.mutateAsync();
      return;
    }
    await setStatus.mutateAsync({
      emoji: emoji || null,
      text: text || null,
      expiresAt: status.minutes ? new Date(Date.now() + status.minutes * 60_000) : null,
    });
  };

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    if (!displayName.trim()) {
      setError('Enter a display name.');
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const updated = await api.typed((c) =>
        c.PATCH('/users/me', {
          body: { display_name: displayName.trim(), bio: bio.trim() || null, avatar_url: avatarUrl.trim() },
        }),
      );
      await saveStatus();
      if (user && instanceUrl) {
        updateInstanceUser(instanceUrl, {
          ...user,
          display_name: updated.display_name ?? '',
          avatar_url: updated.avatar_url ?? null,
        });
      }
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.myStatus(instanceUrl ?? '') });
      if (workspace) {
        await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(workspace.id) });
      }
      onClose();
    } catch (err) {
      setError(toUserMessage(err, 'Could not save your profile.'));
      setSaving(false);
    }
  };

  const handleAvatarUpload = async ([file]: File[]) => {
    if (!workspace) {
      setError('Open a workspace before uploading a photo.');
      return;
    }
    setUploading(true);
    setError(null);
    try {
      const formData = new FormData();
      formData.append('file', file);
      const uploaded = await api.upload<{ url: string }[]>(`/files/upload/${workspace.id}`, formData);
      const url = uploaded[0]?.url;
      if (url) setAvatarUrl(url);
      else setError('Could not upload that photo.');
    } catch {
      setError('Could not upload that photo.');
    } finally {
      setUploading(false);
    }
  };

  const name = displayNameOf(displayName || user?.email);

  return (
    <Modal
      title="Edit profile"
      onClose={onClose}
      size="lg"
      dataQa="profile-modal"
      onSubmit={handleSave}
      dismissLabel="Cancel"
      actions={
        <Button
          type="submit"
          variant="primary"
          loading={saving}
          disabled={!loaded || uploading}
          data-qa="profile-save"
        >
          Save changes
        </Button>
      }
    >
      {isLoading && !loaded ? (
        <div className="flex justify-center py-12 text-accent">
          <Spinner />
        </div>
      ) : (
        <div className="grid gap-6 sm:grid-cols-[1fr_auto]">
          <div className="space-y-5">
            <TextInput
              label="Display name"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              maxLength={100}
              autoComplete="name"
              dataQa="profile-display-name"
              data-autofocus
              required
            />
            <StatusFields
              value={status}
              onChange={setStatusDraft}
              clearsAt={statusChanged ? null : me?.status_expires_at}
            />
            <TextArea
              label="Bio"
              optional
              rows={4}
              value={bio}
              onChange={(e) => setBio(e.target.value)}
              maxLength={500}
              counter
              placeholder="What you work on, where you are, how to reach you"
              dataQa="profile-bio"
            />
            {error && (
              <p role="alert" className="text-sm text-danger">
                {error}
              </p>
            )}
          </div>

          <div className="order-first flex flex-col items-center gap-2 sm:order-none sm:w-40">
            <span className="self-start text-sm font-medium text-fg-dim max-sm:sr-only">Profile photo</span>
            <Avatar userId={user?.id ?? ''} name={name} avatarUrl={avatarUrl || null} size="2xl" />
            <FilePicker
              label="Profile photo file"
              accept="image/*"
              onFiles={handleAvatarUpload}
              disabled={uploading}
              dataQa="profile-avatar-upload"
              className={buttonStyles({ size: 'sm' })}
            >
              {uploading ? <Spinner size="sm" /> : <Camera className="h-3.5 w-3.5" />}
              Upload photo
            </FilePicker>
            {avatarUrl && (
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setAvatarUrl('')}
                data-qa="profile-avatar-remove"
                className="hover:text-danger"
              >
                Remove photo
              </Button>
            )}
          </div>
        </div>
      )}
    </Modal>
  );
}
