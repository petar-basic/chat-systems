import { useRef, useState, type FormEvent } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { X, Smile, Trash2, Upload } from 'lucide-react';
import { api } from '../lib/api';
import { instanceManager } from '../lib/instances';
import { useCustomEmoji, customEmojiKey } from '@/hooks/queries/useCustomEmoji';
import { toUserMessage } from '@/lib/errors';
import { toast } from '@/shared/components/Toast';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  onClose: () => void;
}

export default function CustomEmojiPanel({ workspaceId, instanceUrl, onClose }: Props) {
  const queryClient = useQueryClient();
  const { data: emojis, isLoading } = useCustomEmoji(workspaceId, instanceUrl);
  const [name, setName] = useState('');
  const [file, setFile] = useState<File | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const client = instanceUrl ? instanceManager.get(instanceUrl).api : api;
  const refresh = () => queryClient.invalidateQueries({ queryKey: customEmojiKey(workspaceId) });

  const handleUpload = async (e: FormEvent) => {
    e.preventDefault();
    if (!file || !name.trim()) return;

    setBusy(true);
    setError(null);
    try {
      const form = new FormData();
      form.append('name', name.trim());
      form.append('image', file);
      // Multipart, so this goes through fetch rather than the JSON client.
      const token = await client.getValidToken();
      const res = await fetch(
        `${instanceUrl ?? window.location.origin}/api/workspaces/${workspaceId}/emojis`,
        {
          method: 'POST',
          headers: token ? { Authorization: `Bearer ${token}` } : {},
          credentials: 'include',
          body: form,
        },
      );
      if (!res.ok) {
        const body = await res.json().catch(() => ({ error: res.statusText }));
        throw new Error(body.error || res.statusText);
      }
      setName('');
      setFile(null);
      if (fileRef.current) fileRef.current.value = '';
      await refresh();
      toast.success('Emoji added.');
    } catch (err) {
      setError(toUserMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await client.delete(`/workspaces/${workspaceId}/emojis/${id}`);
      await refresh();
    } catch (err) {
      toast.error(toUserMessage(err));
    }
  };

  return (
    <div className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 flex flex-col border-l border-slate-700/50 bg-slate-900/80">
      <div className="h-14 px-4 flex items-center justify-between border-b border-slate-700/50 shrink-0">
        <h3 className="text-sm font-bold text-white flex items-center gap-2">
          <Smile className="w-4 h-4" />
          Custom emoji
        </h3>
        <button onClick={onClose} className="text-slate-400 hover:text-white transition cursor-pointer">
          <X className="w-4 h-4" />
        </button>
      </div>

      <form onSubmit={handleUpload} className="px-3 py-3 border-b border-slate-700/30 space-y-2">
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="name (used as :name:)"
          data-qa="emoji-name"
          className="w-full px-3 py-2 bg-slate-800 border border-slate-700 rounded-lg text-white text-sm placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-purple-500"
        />
        <input
          ref={fileRef}
          type="file"
          accept="image/png,image/gif,image/webp,image/jpeg"
          onChange={(e) => setFile(e.target.files?.[0] ?? null)}
          data-qa="emoji-file"
          className="w-full text-xs text-slate-400 file:mr-3 file:px-3 file:py-1.5 file:rounded-lg file:border-0 file:bg-slate-700 file:text-slate-200"
        />
        {error && <div className="text-xs text-red-400">{error}</div>}
        <button
          type="submit"
          disabled={busy || !file || !name.trim()}
          data-qa="emoji-upload"
          className="w-full flex items-center justify-center gap-2 px-3 py-2 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white text-sm rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
        >
          <Upload className="w-4 h-4" />
          Add emoji
        </button>
      </form>

      <div className="flex-1 overflow-y-auto px-2 py-2">
        {isLoading ? (
          <div className="flex justify-center py-8">
            <div className="w-5 h-5 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
          </div>
        ) : emojis && emojis.length > 0 ? (
          <ul className="space-y-1">
            {emojis.map((emoji) => (
              <li
                key={emoji.id}
                data-qa="emoji-row"
                className="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-slate-700/30"
              >
                <img src={emoji.url} alt={emoji.name} className="w-6 h-6" />
                <span className="flex-1 text-sm text-slate-200 truncate">:{emoji.name}:</span>
                <button
                  type="button"
                  onClick={() => handleDelete(emoji.id)}
                  aria-label={`Remove :${emoji.name}:`}
                  className="text-slate-400 hover:text-red-400 transition cursor-pointer"
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-center py-8 text-slate-400 text-sm">
            No custom emoji yet. Anything you add here can be typed as <code>:name:</code>.
          </p>
        )}
      </div>
    </div>
  );
}
