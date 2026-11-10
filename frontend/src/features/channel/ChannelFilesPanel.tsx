import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { Download, Files, Search } from 'lucide-react';
import {
  useChannelFiles,
  useChannelFileSearch,
  type ChannelFile,
  type FileKindFilter,
} from '@/features/channel/hooks/useChannelFiles';
import { useWorkspaceStore } from '@/stores/workspace';
import { useDebouncedValue } from '@/shared/hooks/useDebouncedValue';
import { TextInput } from '@/shared/components/Form/TextInput';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';
import { formatBytes } from '@/lib/fileSize';
import { ImageGallery } from '@/features/messaging';
import { fileKind, isRenderableImage } from '@/lib/fileKinds';

const TABS: (FileKindFilter | null)[] = [null, 'image', 'video', 'audio', 'document'];

interface Props {
  channelId: string;
  onClose: () => void;
  onNavigate: (messageId: string) => void;
}

const MEDIA = ['image/', 'video/', 'audio/'];

function inTab(mimeType: string, kind: FileKindFilter | null) {
  if (!kind) return true;
  if (kind === 'document') return !MEDIA.some((prefix) => mimeType.startsWith(prefix));
  return mimeType.startsWith(`${kind}/`);
}

function shortDate(value: string) {
  return new Date(value).toLocaleDateString([], { month: 'short', day: 'numeric' });
}

export default function ChannelFilesPanel({ channelId, onClose, onNavigate }: Props) {
  const { t } = useTranslation('channel');
  const [kind, setKind] = useState<FileKindFilter | null>(null);
  const { data, isLoading, isError, hasNextPage, fetchNextPage, isFetchingNextPage } = useChannelFiles(
    channelId,
    kind,
  );
  const workspaceId = useWorkspaceStore((s) => s.currentWorkspace?.id);
  const [name, setName] = useState('');
  const wanted = useDebouncedValue(name.trim(), 250);
  const named = useChannelFileSearch(workspaceId, channelId, wanted);
  const filtering = wanted.length > 0;
  const listed = useMemo(() => data?.pages.flatMap((page) => page.data) ?? [], [data]);
  const files: ChannelFile[] = filtering
    ? (named.data ?? []).filter((file) => inTab(file.mime_type, kind))
    : listed;
  const loading = filtering ? named.isLoading : isLoading;
  const failed = filtering ? named.isError : isError;
  const images = files.filter((file) => isRenderableImage(file.mime_type));
  const others = files.filter((file) => !isRenderableImage(file.mime_type));
  const [viewing, setViewing] = useState<number | null>(null);

  return (
    <div
      className="w-full lg:w-80 max-lg:fixed max-lg:inset-0 max-lg:z-40 bg-surface border-l border-line/50 flex flex-col"
      data-qa="channel-files-panel"
    >
      <PanelHeader
        title={t('header.files')}
        icon={<Files className="h-4 w-4 text-muted" />}
        onClose={onClose}
      />

      <div className="flex gap-1 overflow-x-auto border-b border-line/50 px-3 py-2" role="tablist">
        {TABS.map((tab) => (
          <button
            key={tab ?? 'all'}
            role="tab"
            aria-selected={kind === tab}
            data-qa={`channel-files-tab-${tab ?? 'all'}`}
            onClick={() => setKind(tab)}
            className={`shrink-0 rounded-full px-2.5 py-1 text-xs transition ${
              kind === tab ? 'bg-purple-600 text-white' : 'text-fg-dim hover:bg-raised'
            }`}
          >
            {t(`files.tabs.${tab ?? 'all'}`)}
          </button>
        ))}
      </div>

      <div className="border-b border-line/50 px-3 py-2">
        <TextInput
          label={t('files.filterLabel')}
          labelHidden
          compact
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={t('files.filterPlaceholder')}
          leading={<Search className="h-3.5 w-3.5" />}
          dataQa="channel-files-filter"
        />
      </div>

      <div className="flex-1 overflow-y-auto">
        {loading && <p className="p-4 text-sm text-muted">{t('files.loading')}</p>}
        {failed && <p className="p-4 text-sm text-danger">{t('files.failed')}</p>}
        {!loading && !failed && files.length === 0 && (
          <div className="flex flex-col items-center justify-center h-full text-muted px-4 text-center">
            <Files className="w-8 h-8 mb-2 text-faint" />
            <p className="text-sm">{filtering ? t('files.noMatch', { name: wanted }) : t('files.empty')}</p>
          </div>
        )}

        {images.length > 0 && (
          <div className="grid grid-cols-3 gap-1 p-3" data-qa="channel-files-images">
            {images.map((image, index) => (
              <button
                key={image.id}
                type="button"
                onClick={() => setViewing(index)}
                data-tooltip={image.filename}
                className="aspect-square overflow-hidden rounded-md border border-line/60 bg-raised/40"
              >
                <img
                  src={image.thumbnail_url ?? image.url}
                  alt={image.filename}
                  loading="lazy"
                  className="h-full w-full object-cover"
                />
              </button>
            ))}
          </div>
        )}

        {others.map((file) => (
          <FileRow key={file.id} file={file} onNavigate={onNavigate} />
        ))}

        {!filtering && hasNextPage && (
          <button
            type="button"
            onClick={() => void fetchNextPage()}
            disabled={isFetchingNextPage}
            className="w-full py-3 text-xs text-accent hover:underline disabled:opacity-50"
          >
            {isFetchingNextPage ? t('files.loading') : t('files.loadMore')}
          </button>
        )}
      </div>

      {viewing !== null && (
        <ImageGallery
          items={images.map((image) => ({
            attachment: image,
            userId: image.user_id,
            createdAt: image.created_at,
          }))}
          index={viewing}
          onIndexChange={setViewing}
          onClose={() => setViewing(null)}
        />
      )}
    </div>
  );
}

function FileRow({ file, onNavigate }: { file: ChannelFile; onNavigate: (messageId: string) => void }) {
  const { t } = useTranslation('channel');
  const { getUser } = useUserCache();
  const kind = fileKind(file.mime_type, file.filename);
  const Icon = kind.icon;
  return (
    <div
      className="group flex items-center gap-3 border-b border-line/50 px-4 py-2.5 hover:bg-raised/20"
      data-qa="channel-file-row"
    >
      <span className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-md ${kind.tone}`}>
        <Icon className="h-4 w-4" />
      </span>
      <button
        type="button"
        onClick={() => onNavigate(file.message_id)}
        className="min-w-0 flex-1 text-left"
        data-tooltip={t('files.showInConversation')}
      >
        <span className="block truncate text-sm text-fg-soft">{file.filename}</span>
        <span className="block truncate text-xs text-muted">
          {displayNameOf(getUser(file.user_id)?.display_name)} · {shortDate(file.created_at)} ·{' '}
          {formatBytes(file.size_bytes)}
        </span>
      </button>
      <a
        href={file.url}
        download={file.filename}
        aria-label={t('files.download', { name: file.filename })}
        className="shrink-0 rounded p-1 text-muted hover:bg-raised hover:text-fg-dim"
      >
        <Download className="h-4 w-4" />
      </a>
    </div>
  );
}
