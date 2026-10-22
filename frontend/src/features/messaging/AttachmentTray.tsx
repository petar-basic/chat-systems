import { AlertCircle, X } from 'lucide-react';
import { formatBytes } from '@/lib/fileSize';
import type { PendingAttachment } from './useComposerAttachments';
import { fileKind } from './fileKinds';

interface Props {
  items: PendingAttachment[];
  onRemove: (localId: string) => void;
}

export function AttachmentTray({ items, onRemove }: Props) {
  if (items.length === 0) return null;
  return (
    <div className="flex gap-2 overflow-x-auto px-3 pt-3" data-qa="attachment-tray">
      {items.map((item) => (
        <TrayItem key={item.localId} item={item} onRemove={() => onRemove(item.localId)} />
      ))}
    </div>
  );
}

function TrayItem({ item, onRemove }: { item: PendingAttachment; onRemove: () => void }) {
  const kind = fileKind(item.mimeType, item.name);
  const Icon = kind.icon;
  return (
    <div
      data-qa="attachment-tray-item"
      data-status={item.status}
      className="group/tray relative flex h-16 shrink-0 items-center gap-2 overflow-hidden rounded-lg border border-line bg-raised/40"
      title={item.name}
    >
      {item.previewUrl ? (
        <img src={item.previewUrl} alt="" className="h-16 w-16 object-cover" />
      ) : (
        <div className="flex w-52 items-center gap-2 px-2">
          <span className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-md ${kind.tone}`}>
            <Icon className="h-4 w-4" />
          </span>
          <span className="min-w-0">
            <span className="block truncate text-sm text-fg-soft">{item.name}</span>
            <span className="block text-xs text-muted">
              {kind.label} · {formatBytes(item.size)}
            </span>
          </span>
        </div>
      )}
      {item.status === 'uploading' && (
        <div className="absolute inset-x-0 bottom-0 h-1 bg-black/30">
          <div
            role="progressbar"
            aria-label={`Uploading ${item.name}`}
            aria-valuenow={Math.round(item.progress * 100)}
            aria-valuemin={0}
            aria-valuemax={100}
            className="h-full bg-purple-500 transition-[width]"
            style={{ width: `${Math.round(item.progress * 100)}%` }}
          />
        </div>
      )}
      {item.status === 'failed' && (
        <span
          className="absolute inset-0 flex items-center justify-center bg-black/60 text-danger"
          title="Upload failed"
        >
          <AlertCircle className="h-5 w-5" />
        </span>
      )}
      <button
        type="button"
        onClick={onRemove}
        aria-label={`Remove ${item.name}`}
        data-qa="attachment-tray-remove"
        className="absolute right-1 top-1 rounded-full bg-black/70 p-0.5 text-white opacity-80 transition hover:opacity-100"
      >
        <X className="h-3 w-3" />
      </button>
    </div>
  );
}
