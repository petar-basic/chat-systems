import { useState } from 'react';
import { FileText, Download } from 'lucide-react';
import type { ParsedAttachment } from '@/lib/attachments';
import { ImageLightbox } from './ImageLightbox';

export function AttachmentCard({ filename, url, isImage }: ParsedAttachment) {
  const [previewing, setPreviewing] = useState(false);

  if (isImage) {
    return (
      <>
        <button
          type="button"
          onClick={() => setPreviewing(true)}
          data-qa="attachment-image"
          className="inline-block mt-1 max-w-sm cursor-zoom-in"
          title={filename}
        >
          <img
            src={url}
            alt={filename}
            loading="lazy"
            className="max-h-80 max-w-full rounded-lg border border-line/60"
          />
        </button>
        {previewing && <ImageLightbox filename={filename} url={url} onClose={() => setPreviewing(false)} />}
      </>
    );
  }

  return (
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      download={filename}
      data-qa="attachment-file"
      className="mt-1 inline-flex items-center gap-3 max-w-sm px-3 py-2 bg-surface border border-line rounded-lg hover:bg-raised/60 transition group"
    >
      <span className="w-9 h-9 rounded-md bg-raised flex items-center justify-center shrink-0">
        <FileText className="w-4 h-4 text-fg-dim" />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block text-sm text-fg-soft truncate">{filename}</span>
        <span className="block text-xs text-muted">Download</span>
      </span>
      <Download className="w-4 h-4 text-muted group-hover:text-fg-dim shrink-0" />
    </a>
  );
}
