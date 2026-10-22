import { useState } from 'react';
import { ExternalLink, Play, RefreshCw, X } from 'lucide-react';
import type { components } from '@/api/schema';
import { appName } from '@/lib/appConnect';
import { AppIcon } from '@/shared/components/AppIcon/AppIcon';
import { embedSrc } from '@/lib/videoEmbed';

type Unfurl = components['schemas']['Unfurl'];
type UnfurlField = components['schemas']['UnfurlField'];

interface LinkPreviewsProps {
  unfurls: Unfurl[];
  onHide?: () => void;
  onRefresh?: () => void;
}

export function LinkPreviews({ unfurls, onHide, onRefresh }: LinkPreviewsProps) {
  const worthShowing = unfurls.filter(
    (unfurl) => unfurl.kind === 'app' || unfurl.kind === 'video' || unfurl.description || unfurl.image_url,
  );
  if (worthShowing.length === 0) return null;
  return (
    <div className="mt-1 flex flex-col gap-1.5" data-qa="link-previews">
      {worthShowing.map((unfurl) =>
        unfurl.kind === 'app' ? (
          <AppCard key={unfurl.url} unfurl={unfurl} onHide={onHide} onRefresh={onRefresh} />
        ) : unfurl.kind === 'video' ? (
          <VideoCard key={unfurl.url} unfurl={unfurl} onHide={onHide} />
        ) : (
          <LinkPreviewCard key={unfurl.url} unfurl={unfurl} onHide={onHide} />
        ),
      )}
    </div>
  );
}

const TONES: Record<string, string> = {
  success: 'bg-green-500/15 text-success',
  info: 'bg-blue-500/15 text-info',
  neutral: 'bg-raised text-fg-dim',
};

function FieldValue({ field }: { field: UnfurlField }) {
  if (field.tone) {
    return (
      <span
        className={`inline-block rounded px-1.5 py-0.5 text-xs font-medium ${TONES[field.tone] ?? TONES.neutral}`}
      >
        {field.value}
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1.5 text-sm text-fg-soft min-w-0">
      {field.avatar_url?.startsWith('https://') && (
        <img src={field.avatar_url} alt="" referrerPolicy="no-referrer" className="h-4 w-4 rounded-full" />
      )}
      <span className="truncate">{field.value}</span>
    </span>
  );
}

function AppCard({
  unfurl,
  onHide,
  onRefresh,
}: {
  unfurl: Unfurl;
  onHide?: () => void;
  onRefresh?: () => void;
}) {
  const provider = appName(unfurl.provider ?? '');
  const [imageFailed, setImageFailed] = useState(false);
  const image = !imageFailed && unfurl.image_url?.startsWith('https://') ? unfurl.image_url : null;
  const fetched = unfurl.fetched_at
    ? new Date(unfurl.fetched_at).toLocaleString([], {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
      })
    : null;
  return (
    <div
      data-qa="app-preview"
      className="group/preview relative max-w-md rounded-lg border border-line/70 bg-surface/40 px-3 py-2"
    >
      <div className="flex items-center gap-1.5 text-xs text-muted">
        <AppIcon provider={unfurl.provider ?? ''} />
        {provider}
      </div>
      <a
        href={unfurl.url}
        target="_blank"
        rel="noopener noreferrer nofollow"
        className="block text-sm font-semibold text-fg hover:underline line-clamp-2"
      >
        {unfurl.title}
      </a>
      {unfurl.subtitle && <div className="text-xs text-fg-dim">{unfurl.subtitle}</div>}
      {unfurl.description && <p className="mt-1 text-xs text-muted line-clamp-2">{unfurl.description}</p>}
      {unfurl.fields && unfurl.fields.length > 0 && (
        <dl className="mt-2 grid grid-cols-2 gap-x-4 gap-y-2">
          {unfurl.fields.map((field) => (
            <div key={field.label} className="min-w-0">
              <dt className="text-xs text-muted">{field.label}</dt>
              <dd>
                <FieldValue field={field} />
              </dd>
            </div>
          ))}
        </dl>
      )}
      {image && (
        <a href={unfurl.url} target="_blank" rel="noopener noreferrer nofollow" className="mt-2 block">
          <img
            src={image}
            alt=""
            loading="lazy"
            referrerPolicy="no-referrer"
            onError={() => setImageFailed(true)}
            data-qa="app-preview-image"
            className="max-h-60 w-full rounded-md border border-line/60 object-cover"
          />
        </a>
      )}
      <div className="mt-2 flex items-center gap-3 text-xs text-muted">
        {fetched && <span>As of {fetched}</span>}
        <a
          href={unfurl.url}
          target="_blank"
          rel="noopener noreferrer nofollow"
          className="ml-auto inline-flex items-center gap-1 text-accent hover:underline"
        >
          Open in {provider} <ExternalLink className="h-3 w-3" />
        </a>
        {onRefresh && (
          <button
            type="button"
            onClick={onRefresh}
            aria-label="Refresh preview"
            title="Refresh"
            data-qa="app-preview-refresh"
            className="rounded p-0.5 hover:bg-raised hover:text-fg-dim"
          >
            <RefreshCw className="h-3 w-3" />
          </button>
        )}
      </div>
      {onHide && (
        <button
          type="button"
          onClick={onHide}
          aria-label="Remove preview"
          title="Remove preview"
          className="absolute right-1 top-1 rounded p-0.5 text-muted opacity-0 transition hover:bg-raised hover:text-fg-dim focus-visible:opacity-100 group-hover/preview:opacity-100"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      )}
    </div>
  );
}

function VideoCard({ unfurl, onHide }: { unfurl: Unfurl; onHide?: () => void }) {
  const [playing, setPlaying] = useState(false);
  const [imageFailed, setImageFailed] = useState(false);
  const src = unfurl.embed ? embedSrc(unfurl.embed) : null;
  const image = !imageFailed && unfurl.image_url?.startsWith('https://') ? unfurl.image_url : null;
  return (
    <div
      data-qa="video-preview"
      className="group/preview relative flex max-w-md flex-col gap-1 border-l-4 border-line pl-3 pr-8 py-1"
    >
      <div className="flex items-center gap-1.5 text-xs font-semibold text-fg-dim">
        <AppIcon provider={unfurl.embed?.provider ?? ''} />
        {unfurl.site_name}
      </div>
      <a
        href={unfurl.url}
        target="_blank"
        rel="noopener noreferrer nofollow"
        className="block text-sm font-semibold text-accent hover:underline line-clamp-2"
      >
        {unfurl.title}
      </a>
      {unfurl.subtitle && <div className="text-xs text-muted">{unfurl.subtitle}</div>}
      {playing && src ? (
        <iframe
          src={src}
          title={unfurl.title}
          allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
          allowFullScreen
          referrerPolicy="strict-origin-when-cross-origin"
          data-qa="video-player"
          className="mt-1 aspect-video w-full rounded-md border border-line/60 bg-black"
        />
      ) : (
        (image || src) && (
          <button
            type="button"
            onClick={() =>
              src ? setPlaying(true) : window.open(unfurl.url, '_blank', 'noopener,noreferrer')
            }
            aria-label={`Play ${unfurl.title}`}
            data-qa="video-play"
            className="relative mt-1 block aspect-video w-full overflow-hidden rounded-md border border-line/60 bg-black"
          >
            {image && (
              <img
                src={image}
                alt=""
                loading="lazy"
                referrerPolicy="no-referrer"
                onError={() => setImageFailed(true)}
                className="h-full w-full object-cover"
              />
            )}
            <span className="absolute inset-0 flex items-center justify-center">
              <span className="flex h-12 w-12 items-center justify-center rounded-full bg-black/70 text-white transition hover:bg-black/85">
                <Play className="h-6 w-6 translate-x-0.5" fill="currentColor" />
              </span>
            </span>
          </button>
        )
      )}
      {onHide && (
        <button
          type="button"
          onClick={onHide}
          aria-label="Remove preview"
          title="Remove preview"
          className="absolute right-1 top-1 rounded p-0.5 text-muted opacity-0 transition hover:bg-raised hover:text-fg-dim focus-visible:opacity-100 group-hover/preview:opacity-100"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      )}
    </div>
  );
}

function LinkPreviewCard({ unfurl, onHide }: { unfurl: Unfurl; onHide?: () => void }) {
  const image = unfurl.image_url?.startsWith('https://') ? unfurl.image_url : null;
  return (
    <div
      data-qa="link-preview"
      className="group/preview relative flex max-w-lg gap-3 border-l-4 border-line pl-3 pr-8 py-1"
    >
      <div className="min-w-0 flex-1">
        {unfurl.site_name && <div className="text-xs font-semibold text-fg-dim">{unfurl.site_name}</div>}
        <a
          href={unfurl.url}
          target="_blank"
          rel="noopener noreferrer nofollow"
          className="block text-sm font-semibold text-accent hover:underline line-clamp-2"
        >
          {unfurl.title}
        </a>
        {unfurl.description && <p className="text-xs text-muted line-clamp-2">{unfurl.description}</p>}
      </div>
      {image && (
        <img
          src={image}
          alt=""
          loading="lazy"
          referrerPolicy="no-referrer"
          className="h-16 w-16 shrink-0 rounded-md border border-line/60 object-cover"
        />
      )}
      {onHide && (
        <button
          type="button"
          onClick={onHide}
          aria-label="Remove preview"
          title="Remove preview"
          data-qa="link-preview-remove"
          className="absolute right-1 top-1 rounded p-0.5 text-muted opacity-0 transition hover:bg-raised hover:text-fg-dim focus-visible:opacity-100 group-hover/preview:opacity-100"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      )}
    </div>
  );
}
