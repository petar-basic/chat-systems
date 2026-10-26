import { useState } from 'react';
import { Download, Paperclip, X } from 'lucide-react';
import type { components } from '@/api/schema';
import { formatBytes } from '@/lib/fileSize';
import { ImageGallery } from './ImageGallery';
import { useGallery } from './galleryContext';
import { fileKind, isRenderableImage, playableMedia } from '@/lib/fileKinds';

type Attachment = components['schemas']['Attachment'];

const GRID_LIMIT = 4;
const MAX_WIDTH = 360;
const MAX_HEIGHT = 320;

function displaySize(width?: number | null, height?: number | null) {
  if (!width || !height) return null;
  const scale = Math.min(1, MAX_WIDTH / width, MAX_HEIGHT / height);
  return { width: Math.max(1, Math.round(width * scale)), height: Math.max(1, Math.round(height * scale)) };
}

function previewOf(image: Attachment): string {
  return image.mime_type === 'image/gif' ? image.url : (image.thumbnail_url ?? image.url);
}

interface Props {
  attachments: Attachment[];
  compact?: boolean;
  onRemove?: (id: string) => void;
}

export function MessageAttachments({ attachments, compact = false, onRemove }: Props) {
  const gallery = useGallery();
  const [viewing, setViewing] = useState<number | null>(null);
  if (attachments.length === 0) return null;

  if (compact) {
    return (
      <div className="mt-1 flex flex-wrap gap-1.5" data-qa="message-attachments-compact">
        {attachments.map((attachment) => (
          <span
            key={attachment.id}
            className="inline-flex max-w-full items-center gap-1 rounded bg-raised/60 px-1.5 py-0.5 text-xs text-fg-dim"
          >
            <Paperclip className="h-3 w-3 shrink-0" />
            <span className="truncate">{attachment.filename}</span>
          </span>
        ))}
      </div>
    );
  }

  const images = onRemove ? [] : attachments.filter((a) => isRenderableImage(a.mime_type));
  const files = attachments.filter((a) => !images.includes(a));
  const open = (image: Attachment) => {
    if (gallery) gallery.open(image.id);
    else setViewing(images.indexOf(image));
  };

  return (
    <div className="mt-1 flex flex-col gap-1.5" data-qa="message-attachments">
      {images.length > 0 && <ImageGrid images={images} onOpen={open} />}
      {files.map((attachment) => {
        const media = onRemove ? null : playableMedia(attachment.mime_type);
        if (media) return <MediaPlayer key={attachment.id} attachment={attachment} kind={media} />;
        return (
          <FileCard
            key={attachment.id}
            attachment={attachment}
            onRemove={onRemove ? () => onRemove(attachment.id) : undefined}
          />
        );
      })}
      {viewing !== null && (
        <ImageGallery
          items={images.map((attachment) => ({ attachment }))}
          index={viewing}
          onIndexChange={setViewing}
          onClose={() => setViewing(null)}
        />
      )}
    </div>
  );
}

function ImageGrid({ images, onOpen }: { images: Attachment[]; onOpen: (image: Attachment) => void }) {
  if (images.length === 1) {
    const [image] = images;
    const size = displaySize(image.width, image.height);
    return (
      <button
        type="button"
        onClick={() => onOpen(image)}
        data-qa="attachment-image"
        className="block w-fit max-w-full cursor-zoom-in"
        title={image.filename}
      >
        <img
          src={previewOf(image)}
          alt={image.filename}
          loading="lazy"
          width={size?.width}
          height={size?.height}
          style={size ? { width: size.width, aspectRatio: `${size.width} / ${size.height}` } : undefined}
          className="max-h-80 max-w-full rounded-lg border border-line/60 bg-raised/40 object-cover"
        />
      </button>
    );
  }

  const shown = images.slice(0, GRID_LIMIT);
  const hidden = images.length - shown.length;
  return (
    <div className="grid w-full max-w-[360px] grid-cols-2 gap-1" data-qa="attachment-grid">
      {shown.map((image, index) => (
        <button
          key={image.id}
          type="button"
          onClick={() => onOpen(image)}
          data-qa="attachment-image"
          title={image.filename}
          className={`relative aspect-square cursor-zoom-in overflow-hidden rounded-lg border border-line/60 ${
            shown.length === 3 && index === 0 ? 'row-span-2 aspect-auto' : ''
          }`}
        >
          <img
            src={previewOf(image)}
            alt={image.filename}
            loading="lazy"
            className="h-full w-full object-cover"
          />
          {hidden > 0 && index === shown.length - 1 && (
            <span
              data-qa="attachment-grid-more"
              className="absolute inset-0 flex items-center justify-center bg-black/55 text-lg font-semibold text-white"
            >
              +{hidden}
            </span>
          )}
        </button>
      ))}
    </div>
  );
}

function MediaPlayer({ attachment, kind }: { attachment: Attachment; kind: 'video' | 'audio' }) {
  const caption = (
    <div className="flex items-center gap-2 text-xs text-muted">
      <span className="truncate text-fg-dim" title={attachment.filename}>
        {attachment.filename}
      </span>
      <span className="shrink-0">· {formatBytes(attachment.size_bytes)}</span>
      <a
        href={attachment.url}
        download={attachment.filename}
        aria-label={`Download ${attachment.filename}`}
        className="ml-auto shrink-0 rounded p-0.5 hover:bg-raised hover:text-fg-dim"
      >
        <Download className="h-3.5 w-3.5" />
      </a>
    </div>
  );
  if (kind === 'video') {
    return (
      <div className="flex w-full max-w-[480px] flex-col gap-1" data-qa="attachment-video">
        <video
          src={attachment.url}
          controls
          preload="metadata"
          playsInline
          className="aspect-video w-full rounded-lg border border-line/60 bg-black"
        />
        {caption}
      </div>
    );
  }
  return (
    <div
      className="flex w-full max-w-sm flex-col gap-1 rounded-lg border border-line bg-surface px-3 py-2"
      data-qa="attachment-audio"
    >
      {caption}
      <audio src={attachment.url} controls preload="metadata" className="w-full" />
    </div>
  );
}

function FileCard({ attachment, onRemove }: { attachment: Attachment; onRemove?: () => void }) {
  const kind = fileKind(attachment.mime_type, attachment.filename);
  const Icon = kind.icon;
  const body = (
    <>
      <span className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-md ${kind.tone}`}>
        <Icon className="h-4 w-4" />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm text-fg-soft">{attachment.filename}</span>
        <span className="block text-xs text-muted">
          {kind.label} · {formatBytes(attachment.size_bytes)}
        </span>
      </span>
    </>
  );
  const frame =
    'group mt-0 flex w-full max-w-sm items-center gap-3 rounded-lg border border-line bg-surface px-3 py-2 transition';

  if (onRemove) {
    return (
      <div className={frame} data-qa="attachment-file">
        {body}
        <button
          type="button"
          onClick={onRemove}
          aria-label={`Remove ${attachment.filename}`}
          className="shrink-0 rounded p-1 text-muted hover:bg-raised hover:text-danger"
        >
          <X className="h-4 w-4" />
        </button>
      </div>
    );
  }
  return (
    <a
      href={attachment.url}
      target="_blank"
      rel="noopener noreferrer"
      download={attachment.filename}
      data-qa="attachment-file"
      className={`${frame} hover:bg-raised/60`}
    >
      {body}
      <Download className="h-4 w-4 shrink-0 text-muted group-hover:text-fg-dim" />
    </a>
  );
}
