import { X } from 'lucide-react';
import type { components } from '@/api/schema';

type Unfurl = components['schemas']['Unfurl'];

interface LinkPreviewsProps {
  unfurls: Unfurl[];
  onHide?: () => void;
}

export function LinkPreviews({ unfurls, onHide }: LinkPreviewsProps) {
  return (
    <div className="mt-1 flex flex-col gap-1.5" data-qa="link-previews">
      {unfurls.map((unfurl) => (
        <LinkPreviewCard key={unfurl.url} unfurl={unfurl} onHide={onHide} />
      ))}
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
