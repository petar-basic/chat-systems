import { useState, useRef, type FormEvent } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useCurrentUser } from '../hooks/queries/useAuth';
import { useInstanceStore } from '../stores/instances';
import { instanceManager } from '../lib/instances';
import { api } from '../lib/api';
import { X, Save, Camera, User } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { QUERY_KEYS } from '@/shared/constants';

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
        avatar_url: avatarUrl.trim() || null,
      });

      queryClient.setQueryData(QUERY_KEYS.currentUser(), (old: typeof user) => {
        if (!old) return old;
        return {
          ...old,
          display_name: updated.display_name,
          avatar_url: updated.avatar_url,
        };
      });

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

    setUploading(true);
    try {
      const formData = new FormData();
      formData.append('file', file);

      const { useInstanceStore } = await import('../stores/instances');
      const activeUrl = useInstanceStore.getState().activeInstanceUrl;
      const baseUrl = activeUrl && activeUrl !== window.location.origin ? `${activeUrl}/api` : '/api';
      const res = await fetch(`${baseUrl}/files/upload/avatars`, {
        method: 'POST',
        credentials: 'include',
        body: formData,
      });

      if (res.ok) {
        const uploaded = await res.json();
        setAvatarUrl(uploaded.url);
      } else {
        setError('Failed to upload avatar');
      }
    } catch {
      setError('Failed to upload avatar');
    } finally {
      setUploading(false);
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
  };

  const initials = displayName
    ? displayName.charAt(0).toUpperCase()
    : user?.email?.charAt(0).toUpperCase() || '?';

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
            {avatarUrl ? (
              <img src={avatarUrl} alt="Avatar" className="w-16 h-16 rounded-full object-cover" />
            ) : (
              <div className="w-16 h-16 rounded-full bg-purple-600 flex items-center justify-center text-2xl font-bold text-white">
                {initials}
              </div>
            )}
            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              disabled={uploading}
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

        <div>
          <label htmlFor="profile-avatar-url" className="block text-sm font-medium text-slate-300 mb-1.5">
            Avatar URL
          </label>
          <input
            id="profile-avatar-url"
            type="url"
            value={avatarUrl}
            onChange={(e) => setAvatarUrl(e.target.value)}
            className="w-full px-3 py-2.5 bg-slate-700/50 border border-slate-600 rounded-lg text-white text-sm placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
            placeholder="https://example.com/avatar.png"
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
    </Modal>
  );
}
