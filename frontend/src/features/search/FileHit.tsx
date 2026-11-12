import type { ReactNode } from 'react';
import { Download } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { components } from '@/api/schema';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';
import { formatBytes } from '@/lib/fileSize';
import { formatDateTime } from '@/lib/datetime';
import { wordRanges } from '@/lib/fold';
import { FILE_KINDS, isRenderableImage } from '@/lib/fileKinds';

export type FileSearchHit = components['schemas']['FileSearchHit'];
type SnippetPart = components['schemas']['SnippetPart'];

interface Props {
  hit: FileSearchHit;
  query: string;
  context?: string;
  onOpen?: () => void;
}

function HighlightedName({ name, query }: { name: string; query: string }) {
  const ranges = wordRanges(name, query);
  if (ranges.length === 0) return <>{name}</>;
  const parts: ReactNode[] = [];
  let at = 0;
  for (const [start, end] of ranges) {
    parts.push(name.slice(at, start));
    parts.push(
      <mark key={start} className="bg-warning/25 text-fg rounded-sm" data-qa="file-hit-match">
        {name.slice(start, end)}
      </mark>,
    );
    at = end;
  }
  parts.push(name.slice(at));
  return <>{parts}</>;
}

function Snippet({ parts }: { parts: SnippetPart[] }) {
  return (
    <span className="mt-0.5 line-clamp-2 text-xs text-fg-dim" data-qa="file-hit-snippet">
      {parts.map((part, index) =>
        part.hit ? (
          <mark key={index} className="bg-warning/25 text-fg rounded-sm" data-qa="file-hit-snippet-match">
            {part.text}
          </mark>
        ) : (
          <span key={index}>{part.text}</span>
        ),
      )}
    </span>
  );
}

export function FileHit({ hit, query, context, onOpen }: Props) {
  const { t } = useTranslation('search');
  const { getUser } = useUserCache();
  const kind = FILE_KINDS[hit.kind];
  const Icon = kind.icon;
  const preview = isRenderableImage(hit.mime_type) ? (hit.thumbnail_url ?? hit.url) : null;
  const meta = [
    formatBytes(hit.size_bytes),
    displayNameOf(getUser(hit.user_id)?.display_name),
    context,
    formatDateTime(hit.created_at),
  ].filter(Boolean);

  return (
    <div className="group relative flex items-center rounded-lg hover:bg-raised/30" data-qa="file-hit">
      <button
        type="button"
        onClick={onOpen}
        disabled={!onOpen}
        data-tooltip={preview ? t('panel.openInGallery') : t('panel.showInConversation')}
        className="flex min-w-0 flex-1 items-center gap-3 px-3 py-2.5 text-left disabled:cursor-default"
      >
        {preview ? (
          <img
            src={preview}
            alt=""
            loading="lazy"
            className="h-9 w-9 shrink-0 rounded-md border border-line/60 object-cover"
          />
        ) : (
          <span
            className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-md ${kind.tone}`}
            data-qa={`file-hit-kind-${hit.kind}`}
          >
            <Icon className="h-4 w-4" />
          </span>
        )}
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm text-fg-soft">
            <HighlightedName name={hit.filename} query={query} />
          </span>
          {hit.snippet && hit.snippet.length > 0 && <Snippet parts={hit.snippet} />}
          <span className="block truncate text-xs text-muted">{meta.join(' · ')}</span>
        </span>
      </button>
      <a
        href={hit.url}
        download={hit.filename}
        aria-label={t('panel.download', { name: hit.filename })}
        className="absolute right-2 rounded p-1 text-muted opacity-0 transition hover:bg-raised hover:text-fg-dim focus:opacity-100 group-hover:opacity-100"
      >
        <Download className="h-4 w-4" />
      </a>
    </div>
  );
}
