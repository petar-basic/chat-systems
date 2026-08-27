import { useState, useRef, type FormEvent } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useCurrentUser } from '../hooks/queries/useAuth';
import { useInstanceStore } from '../stores/instances';
import { useWorkspaceStore } from '../stores/workspace';
import { instanceManager } from '../lib/instances';
import { api } from '../lib/api';
import { X, Save, Camera, User } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { displayNameOf } from '@/lib/userHelpers';
import { QUERY_KEYS } from '@/shared/constants';
import StatusEditor from './StatusEditor';
import TwoFactorPanel from './TwoFactorPanel';
import PushNotificationsPanel from './PushNotificationsPanel';
import EmailNotificationsPanel from './EmailNotificationsPanel';

interface Props {
  onClose: () => void;
}

interface UserProfile {
  id: string;
  email: string;
  display_name: string;
  avatar_url: string | null;
  bio: string | null;
  timezone: string | null;
}

function useActiveApi() {
  const activeInstanceUrl = useInstanceStore((s) => s.activeInstanceUrl);
  return activeInstanceUrl ? instanceManager.get(activeInstanceUrl).api : api;
}

export default function UserProfilePanel({ onClose }: Props) {
  const queryClient = useQueryClient();
  const activeApi = useActiveApi();
  const activeInstanceUrl = useInstanceStore((s) => s.activeInstanceUrl);
  const updateInstanceUser = useInstanceStore((s) => s.updateInstanceUser);
  const currentWorkspace = useWorkspaceStore((s) => s.currentWorkspace);
  const { data: user } = useCurrentUser();
  const [displayName, setDisplayName] = useState(user?.display_name || '');
  const [bio, setBio] = useState('');
  const [avatarUrl, setAvatarUrl] = useState(user?.avatar_url || '');
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [uploading, setUploading] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [loaded, setLoaded] = useState(false);

  if (!loaded) {
    api
      .get<UserProfile>('/users/me')
      .then((profile) => {
        setDisplayName(profile.display_name || '');
        setBio(profile.bio || '');
        setAvatarUrl(profile.avatar_url || '');
        setLoaded(true);
      })
      .catch(() => setLoaded(true));
  }

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    if (!displayName.trim()) return;

    setSaving(true);
    setError(null);
    setSaved(false);
    try {
      const updated = await activeApi.patch<UserProfile>('/users/me', {
        display_name: displayName.trim(),
        bio: bio.trim() || null,
        avatar_url: avatarUrl.trim(),
      });

      if (user && activeInstanceUrl) {
        updateInstanceUser(activeInstanceUrl, {
          ...user,
          display_name: updated.display_name,
          avatar_url: updated.avatar_url,
        });
      }
      if (currentWorkspace) {
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaceMembers(currentWorkspace.id) });
      }

      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Failed to save profile';
      setError(msg);
    } finally {
      setSaving(false);
    }
  };

  const handleAvatarUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    if (!currentWorkspace) {
      setError('Open a workspace before uploading an avatar');
      return;
    }

    setUploading(true);
    setError(null);
    try {
      const formData = new FormData();
      formData.append('file', file);

      const uploaded = await activeApi.upload<{ url: string }[]>(
        `/files/upload/${currentWorkspace.id}`,
        formData,
      );
      const url = uploaded[0]?.url;
      if (url) setAvatarUrl(url);
      else setError('Failed to upload avatar');
    } catch {
      setError('Failed to upload avatar');
    } finally {
      setUploading(false);
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
  };

  return (
    <Modal
      title="Profile Settings"
      onClose={onClose}
      dataQa="profile-modal"
      className="bg-slate-800 border border-slate-700 rounded-2xl shadow-2xl w-full max-w-md"
    >
      <div className="px-6 py-4 flex items-center justify-between border-b border-slate-700/50">
        <h2 className="text-lg font-bold text-white flex items-center gap-2">
          <User className="w-5 h-5" />
          Profile Settings
        </h2>
        <button onClick={onClose} className="text-slate-400 hover:text-white transition cursor-pointer">
          <X className="w-5 h-5" />
        </button>
      </div>

      <form onSubmit={handleSave} className="p-6 space-y-5">
        <div className="flex items-center gap-4">
          <div className="relative">
            <Avatar
              userId={user?.id ?? ''}
              name={displayNameOf(displayName || user?.email)}
              avatarUrl={avatarUrl || null}
              size="xl"
            />
            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              disabled={uploading}
              aria-label="Upload a new photo"
              data-qa="profile-avatar-upload"
              className="absolute -bottom-1 -right-1 w-7 h-7 bg-slate-700 hover:bg-slate-600 border-2 border-slate-800 rounded-full flex items-center justify-center transition cursor-pointer"
            >
              {uploading ? (
                <div className="w-3 h-3 border-2 border-white/30 border-t-white rounded-full animate-spin" />
              ) : (
                <Camera className="w-3.5 h-3.5 text-slate-300" />
              )}
            </button>
            <input
              ref={fileInputRef}
              type="file"
              accept="image/*"
              className="hidden"
              onChange={handleAvatarUpload}
            />
          </div>
          <div className="flex-1 min-w-0">
            <div className="text-sm font-medium text-white truncate">
              {user?.display_name || 'No name set'}
            </div>
            <div className="text-xs text-slate-400 truncate">{user?.email}</div>
            {avatarUrl && (
              <button
                type="button"
                onClick={() => setAvatarUrl('')}
                data-qa="profile-avatar-remove"
                className="mt-1 text-xs text-slate-400 hover:text-red-400 transition cursor-pointer"
              >
                Remove photo
              </button>
            )}
          </div>
        </div>

        <div>
          <label htmlFor="profile-display-name" className="block text-sm font-medium text-slate-300 mb-1.5">
            Display Name
          </label>
          <input
            id="profile-display-name"
            type="text"
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            className="w-full px-3 py-2.5 bg-slate-700/50 border border-slate-600 rounded-lg text-white text-sm placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
            required
          />
        </div>

        <StatusEditor instanceUrl={activeInstanceUrl ?? undefined} workspaceId={currentWorkspace?.id} />

        <div>
          <label htmlFor="profile-bio" className="block text-sm font-medium text-slate-300 mb-1.5">
            Bio
          </label>
          <textarea
            id="profile-bio"
            value={bio}
            onChange={(e) => setBio(e.target.value)}
            rows={3}
            className="w-full px-3 py-2.5 bg-slate-700/50 border border-slate-600 rounded-lg text-white text-sm placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500 resize-none"
            placeholder="Tell others about yourself..."
          />
        </div>

        {error && (
          <div className="text-sm text-red-400 bg-red-500/10 border border-red-500/30 rounded-lg px-3 py-2">
            {error}
          </div>
        )}

        {saved && (
          <div className="text-sm text-green-400 bg-green-500/10 border border-green-500/30 rounded-lg px-3 py-2">
            Profile saved successfully.
          </div>
        )}

        <div className="flex justify-end gap-2 pt-2">
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-2.5 text-sm text-slate-400 hover:text-white transition cursor-pointer"
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={saving || !displayName.trim()}
            className="flex items-center gap-2 px-4 py-2.5 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white text-sm font-medium rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
          >
            {saving ? (
              <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
            ) : (
              <>
                <Save className="w-4 h-4" />
                Save Profile
              </>
            )}
          </button>
        </div>
      </form>

      <section className="border-t border-slate-700 pt-5 mt-5 space-y-5">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-500">Notifications</h3>
        <PushNotificationsPanel />
        <EmailNotificationsPanel />
      </section>

      <section className="border-t border-slate-700 pt-5 mt-5">
        <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-500 mb-3">Security</h3>
        <TwoFactorPanel />
      </section>
    </Modal>
  );
}
