import { useState, type FormEvent } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { TextArea } from '@/shared/components/Form/TextArea';
import { toast } from '@/shared/components/Toast';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { QUERY_KEYS } from '@/shared/constants';
import { useCurrentUser } from '@/features/instance';
import { useAccount } from '@/features/profile/hooks/useProfile';
import { useInstanceStore } from '@/stores/instances';
import { displayNameOf } from '@/lib/userHelpers';
import { toUserMessage } from '@/lib/errors';
import type { components } from '@/api/schema';
import { ProfilePhotoField } from './ProfilePhotoField';

type Account = components['schemas']['UserPublic'];

export default function AccountPanel() {
  const queryClient = useQueryClient();
  const instanceUrl = useInstanceStore((s) => s.activeInstanceUrl) ?? undefined;
  const updateInstanceUser = useInstanceStore((s) => s.updateInstanceUser);
  const { data: user } = useCurrentUser();
  const { data: account } = useAccount(instanceUrl);

  const [loaded, setLoaded] = useState(false);
  const [displayName, setDisplayName] = useState('');
  const [bio, setBio] = useState('');
  const [avatarUrl, setAvatarUrl] = useState('');
  const [saving, setSaving] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!loaded && account) {
    setLoaded(true);
    setDisplayName(account.display_name ?? '');
    setBio(account.bio ?? '');
    setAvatarUrl(account.avatar_url ?? '');
  }

  const applyAccount = async (updated: Account) => {
    queryClient.setQueryData(QUERY_KEYS.account(instanceUrl ?? ''), updated);
    if (user && instanceUrl) {
      updateInstanceUser(instanceUrl, {
        ...user,
        display_name: updated.display_name ?? '',
        avatar_url: updated.avatar_url ?? null,
      });
    }
    await queryClient.invalidateQueries({
      predicate: (q) => q.queryKey[0] === 'workspaces' && q.queryKey[2] === 'members',
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
      const updated = await getApiForInstance(instanceUrl).typed((c) =>
        c.PATCH('/users/me', {
          body: { display_name: displayName.trim(), bio: bio.trim() || null, avatar_url: avatarUrl.trim() },
        }),
      );
      await applyAccount(updated);
      toast.success('Account saved.');
    } catch (err) {
      setError(toUserMessage(err, 'Could not save your account.'));
    } finally {
      setSaving(false);
    }
  };

  const handlePhoto = async (file: File) => {
    setError(null);
    setUploading(true);
    try {
      const formData = new FormData();
      formData.append('file', file);
      const updated = await getApiForInstance(instanceUrl).upload<Account>('/users/me/avatar', formData);
      setAvatarUrl(updated.avatar_url ?? '');
      await applyAccount(updated);
      toast.success('Photo updated.');
    } catch (err) {
      setError(toUserMessage(err, 'Could not upload that photo.'));
    } finally {
      setUploading(false);
    }
  };

  return (
    <form onSubmit={(e) => void handleSave(e)} data-qa="account-panel">
      <p className="mb-4 text-sm text-muted">
        Your name, photo and bio in every workspace where you have not set your own.
      </p>
      <div className="grid gap-6 sm:grid-cols-[1fr_auto]">
        <div className="space-y-5">
          <TextInput
            label="Display name"
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            maxLength={100}
            autoComplete="name"
            dataQa="account-display-name"
            required
          />
          <TextArea
            label="Bio"
            optional
            rows={4}
            value={bio}
            onChange={(e) => setBio(e.target.value)}
            maxLength={500}
            counter
            dataQa="account-bio"
          />
          {error && (
            <p role="alert" className="text-sm text-danger">
              {error}
            </p>
          )}
          <Button
            type="submit"
            variant="primary"
            loading={saving}
            disabled={!loaded || uploading}
            data-qa="account-save"
          >
            Save account
          </Button>
        </div>
        <ProfilePhotoField
          userId={user?.id ?? ''}
          name={displayNameOf(displayName || user?.email)}
          shown={avatarUrl || null}
          uploading={uploading}
          resetLabel={avatarUrl ? 'Remove photo' : null}
          qaPrefix="account"
          onFile={(file) => void handlePhoto(file)}
          onReset={() => setAvatarUrl('')}
        />
      </div>
    </form>
  );
}
