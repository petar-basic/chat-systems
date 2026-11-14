import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
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
import { useUserCache } from '@/stores/users';
import {
  ProfileFieldInputs,
  changedValues,
  editableFields,
  useProfileFields,
  useSetProfileValues,
  type FieldValues,
} from '@/features/people';
import { displayNameOf } from '@/lib/userHelpers';
import { toUserMessage } from '@/lib/errors';
import { ProfilePhotoField } from './ProfilePhotoField';
import { StatusFields } from './StatusFields';
import { sameStatus, type StatusDraft } from './statusDraft';

const EMPTY_STATUS: StatusDraft = { emoji: '', text: '', minutes: null };

interface Props {
  onClose: () => void;
}

const NO_VALUES: FieldValues = {};

export function EditProfileModal({ onClose }: Props) {
  const { t } = useTranslation('profile');
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const instanceUrl = workspace?.instanceUrl;
  const { data: user } = useCurrentUser();
  const { data: account } = useAccount(instanceUrl);
  const { data: profile } = useWorkspaceProfile(workspace?.id, instanceUrl);
  const save = useSaveWorkspaceProfile(workspace?.id, instanceUrl);
  const photo = usePhotoUpload(instanceUrl, workspace?.id);
  const { fields } = useProfileFields(workspace?.id, instanceUrl);
  const editable = editableFields(fields, false);
  const savedFields =
    useUserCache((s) => (user ? s.getUser(user.id)?.profile_fields : undefined)) ?? NO_VALUES;
  const setValues = useSetProfileValues(workspace?.id ?? '', instanceUrl);
  const [fieldDraft, setFieldDraft] = useState<FieldValues>(NO_VALUES);

  const [loaded, setLoaded] = useState(false);
  const [displayName, setDisplayName] = useState('');
  const [title, setTitle] = useState('');
  const [pronouns, setPronouns] = useState('');
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
    setTitle(profile.title ?? '');
    setPronouns(profile.pronouns ?? '');
    setBio(profile.bio ?? '');
    setAvatarUrl(profile.avatar_url ?? '');
    setStatusDraft(current);
    setSavedStatus(current);
    setFieldDraft(savedFields);
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
        title: title.trim(),
        pronouns: pronouns.trim(),
        bio: bio.trim(),
        avatar_url: avatarUrl.trim(),
      });
      await saveStatus();
      const changes = changedValues(savedFields, fieldDraft, editable);
      if (user && Object.keys(changes).length > 0) {
        await setValues.mutateAsync({ userId: user.id, values: changes });
      }
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
    if (!url) {
      setError('Could not upload that photo.');
      return;
    }
    setAvatarUrl(url);
    void save.refresh();
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
      footerStart={
        error && (
          <p role="alert" className="text-sm text-danger" data-qa="profile-error">
            {error}
          </p>
        )
      }
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
      {!loaded ? (
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
            <div className="grid gap-5 sm:grid-cols-[1fr_10rem]">
              <TextInput
                label="Title"
                optional
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder={account?.title || 'What you do, e.g. Head of Finance'}
                maxLength={100}
                autoComplete="organization-title"
                dataQa="profile-title"
              />
              <TextInput
                label="Pronouns"
                optional
                value={pronouns}
                onChange={(e) => setPronouns(e.target.value)}
                placeholder={account?.pronouns || 'e.g. they/them'}
                maxLength={30}
                dataQa="profile-pronouns"
              />
            </div>
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
            {editable.length > 0 && (
              <section className="space-y-3" data-qa="profile-fields-section">
                <h3 className="text-sm font-semibold text-fg">{t('fields.heading')}</h3>
                <ProfileFieldInputs
                  fields={editable}
                  values={fieldDraft}
                  onChange={(fieldId, value) => setFieldDraft((d) => ({ ...d, [fieldId]: value }))}
                />
              </section>
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
