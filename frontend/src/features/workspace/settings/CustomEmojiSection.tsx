import { useRef, useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Trans } from '@/shared/components/Trans/Trans';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { useQueryClient } from '@tanstack/react-query';
import { Trash2, Upload } from 'lucide-react';
import { useCustomEmoji, customEmojiKey } from '@/features/workspace/hooks/useCustomEmoji';
import { useMediaLinks } from '@/shared/hooks/useMediaUrl';
import { toUserMessage } from '@/lib/errors';
import { toast } from '@/shared/components/Toast';
import { TextInput } from '@/shared/components/Form/TextInput';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
}

export function CustomEmojiSection({ workspaceId, instanceUrl }: Props) {
  const { t } = useTranslation('workspace');
  const queryClient = useQueryClient();
  const { data: emojis, isLoading } = useCustomEmoji(workspaceId, instanceUrl);
  const [name, setName] = useState('');
  const [file, setFile] = useState<File | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  const client = getApiForInstance(instanceUrl);
  const mediaLink = useMediaLinks();
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
      await client.upload(`/workspaces/${workspaceId}/emojis`, form);
      setName('');
      setFile(null);
      if (fileRef.current) fileRef.current.value = '';
      await refresh();
      toast.success(t('settings.customEmoji.added'));
    } catch (err) {
      setError(toUserMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await client.typed((c) =>
        c.DELETE('/workspaces/{ws_id}/emojis/{emoji_id}', {
          params: { path: { ws_id: workspaceId, emoji_id: id } },
        }),
      );
      await refresh();
    } catch (err) {
      toast.error(toUserMessage(err));
    }
  };

  return (
    <div className="-mx-4 flex flex-col">
      <form onSubmit={handleUpload} className="px-3 py-3 border-b border-line/30 space-y-2">
        <TextInput
          label={t('settings.customEmoji.name')}
          labelHidden
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={t('settings.customEmoji.namePlaceholder')}
          dataQa="emoji-name"
        />
        <input
          aria-label={t('settings.customEmoji.image')}
          ref={fileRef}
          type="file"
          accept="image/png,image/gif,image/webp,image/jpeg"
          onChange={(e) => setFile(e.target.files?.[0] ?? null)}
          data-qa="emoji-file"
          className="w-full text-xs text-muted file:mr-3 file:px-3 file:py-1.5 file:rounded-lg file:border-0 file:bg-raised file:text-fg-soft"
        />
        {error && <div className="text-xs text-danger">{error}</div>}
        <Button
          variant="primary"
          fullWidth
          type="submit"
          disabled={busy || !file || !name.trim()}
          data-qa="emoji-upload"
        >
          <Upload className="w-4 h-4" />
          {t('settings.customEmoji.add')}
        </Button>
      </form>

      <div className="flex-1 overflow-y-auto px-2 py-2">
        {isLoading ? (
          <div className="flex justify-center py-8">
            <Spinner className="text-accent" />
          </div>
        ) : emojis && emojis.length > 0 ? (
          <ul className="space-y-1">
            {emojis.map((emoji) => (
              <li
                key={emoji.id}
                data-qa="emoji-row"
                className="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-raised/30"
              >
                <img src={mediaLink(emoji.url)} alt={emoji.name} className="w-6 h-6" />
                <span className="flex-1 text-sm text-fg-soft truncate">:{emoji.name}:</span>
                <button
                  type="button"
                  onClick={() => handleDelete(emoji.id)}
                  aria-label={t('settings.customEmoji.removeNamed', { name: emoji.name })}
                  data-tooltip={t('settings.customEmoji.removeNamed', { name: emoji.name })}
                  className="text-muted hover:text-danger transition cursor-pointer"
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-center py-8 text-muted text-sm">
            <Trans t={t} i18nKey="settings.customEmoji.empty" components={{ code: <code /> }} />
          </p>
        )}
      </div>
    </div>
  );
}
