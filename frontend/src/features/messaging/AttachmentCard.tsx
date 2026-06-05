import { FileText, Download } from 'lucide-react';
import type { ParsedAttachment } from '@/lib/attachments';

export function AttachmentCard({ filename, url, isImage }: ParsedAttachment) {
  if (isImage) {
    return (
      <a
        href={url}
        target="_blank"
        rel="noopener noreferrer"
        data-qa="attachment-image"
        className="inline-block mt-1 max-w-sm"
        title={filename}
      >
        <img
          src={url}
          alt={filename}
          loading="lazy"
          className="max-h-80 max-w-full rounded-lg border border-slate-700/60"
        />
      </a>
    );
  }

  return (
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      download={filename}
      data-qa="attachment-file"
      className="mt-1 inline-flex items-center gap-3 max-w-sm px-3 py-2 bg-slate-800 border border-slate-700 rounded-lg hover:bg-slate-700/60 transition group"
    >
      <span className="w-9 h-9 rounded-md bg-slate-700 flex items-center justify-center shrink-0">
        <FileText className="w-4 h-4 text-slate-300" />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block text-sm text-slate-200 truncate">{filename}</span>
        <span className="block text-xs text-slate-400">Download</span>
      </span>
      <Download className="w-4 h-4 text-slate-400 group-hover:text-slate-300 shrink-0" />
    </a>
  );
}
