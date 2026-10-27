import { useState, type FormEvent } from 'react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { TextArea } from '@/shared/components/Form/TextArea';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { useCurrentUser } from '@/features/instance';
import {
  useAccount,
  useSaveWorkspaceProfile,
  useWorkspaceProfile,
} from '@/features/profile/hooks/useProfile';
import { usePhotoUpload } from '@/features/profile/hooks/usePhotoUpload';
import { useWorkspaceStore } from '@/stores/workspace';
import { displayNameOf } from '@/lib/userHelpers';
import { toUserMessage } from '@/lib/errors';
import { ProfilePhotoField } from './ProfilePhotoField';
import { StatusFields } from './StatusFields';
import { sameStatus, type StatusDraft } from './statusDraft';

const EMPTY_STATUS: StatusDraft = { emoji: '', text: '', minutes: null };

interface Props {
  onClose: () => void;
}

export function EditProfileModal({ onClose }: Props) {
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const instanceUrl = workspace?.instanceUrl;
  const { data: user } = useCurrentUser();
  const { data: account } = useAccount(instanceUrl);
  const { data: profile, isLoading } = useWorkspaceProfile(workspace?.id, instanceUrl);
  const save = useSaveWorkspaceProfile(workspace?.id, instanceUrl);
  const photo = usePhotoUpload(instanceUrl, workspace?.id);

  const [loaded, setLoaded] = useState(false);
  const [displayName, setDisplayName] = useState('');
  const [bio, setBio] = useState('');
  const [avatarUrl, setAvatarUrl] = useState('');
  const [status, setStatusDraft] = useState<StatusDraft>(EMPTY_STATUS);
  const [savedStatus, setSavedStatus] = useState<StatusDraft>(EMPTY_STATUS);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!loaded && profile) {
    const current = { emoji: profile.status_emoji ?? '', text: profile.status_text ?? '', minutes: null };
    setLoaded(true);
    setDisplayName(profile.display_name ?? '');
    setBio(profile.bio ?? '');
    setAvatarUrl(profile.avatar_url ?? '');
    setStatusDraft(current);
    setSavedStatus(current);
  }

  const statusChanged = !sameStatus(status, savedStatus);

  const saveStatus = async () => {
    if (!statusChanged) return;
    const emoji = status.emoji.trim();
    const text = status.text.trim();
    if (!emoji && !text) {
      await save.clearStatus.mutateAsync();
      return;
    }
    await save.setStatus.mutateAsync({
      emoji: emoji || null,
      text: text || null,
      expiresAt: status.minutes ? new Date(Date.now() + status.minutes * 60_000) : null,
    });
  };

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setError(null);
    try {
      await save.profile.mutateAsync({
        display_name: displayName.trim(),
        bio: bio.trim(),
        avatar_url: avatarUrl.trim(),
      });
      await saveStatus();
      await save.refresh();
      onClose();
    } catch (err) {
      setError(toUserMessage(err, 'Could not save your profile.'));
      setSaving(false);
    }
  };

  const handlePhoto = async (file: File) => {
    setError(null);
    const url = await photo.upload(file);
    if (url) setAvatarUrl(url);
    else setError('Could not upload that photo.');
  };

  const accountName = account?.display_name ?? '';
  const name = displayNameOf(displayName || accountName || user?.email);

  return (
    <Modal
      title="Edit profile"
      description={
        workspace && (
          <>
            How you appear in <strong className="text-fg">{workspace.name}</strong>. Anything left empty shows
            your account's.
          </>
        )
      }
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
          disabled={!loaded || photo.uploading}
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
              optional
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              placeholder={accountName}
              maxLength={100}
              autoComplete="name"
              dataQa="profile-display-name"
              data-autofocus
            />
            <StatusFields
              value={status}
              onChange={setStatusDraft}
              clearsAt={statusChanged ? null : profile?.status_expires_at}
            />
            <TextArea
              label="Bio"
              optional
              rows={4}
              value={bio}
              onChange={(e) => setBio(e.target.value)}
              maxLength={500}
              counter
              placeholder={account?.bio || 'What you work on, where you are, how to reach you'}
              dataQa="profile-bio"
            />
            {error && (
              <p role="alert" className="text-sm text-danger">
                {error}
              </p>
            )}
          </div>

          <ProfilePhotoField
            userId={user?.id ?? ''}
            name={name}
            shown={avatarUrl || account?.avatar_url || null}
            uploading={photo.uploading}
            resetLabel={avatarUrl ? (account?.avatar_url ? 'Use account photo' : 'Remove photo') : null}
            qaPrefix="profile"
            onFile={(file) => void handlePhoto(file)}
            onReset={() => setAvatarUrl('')}
          />
        </div>
      )}
    </Modal>
  );
}
