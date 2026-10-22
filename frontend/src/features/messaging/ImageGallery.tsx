import { useEffect, useRef, useState } from 'react';
import { ChevronLeft, ChevronRight, Download, ExternalLink, X } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';
import { formatDateTime } from '@/lib/datetime';
import type { GalleryItem } from './galleryContext';

const SWIPE_DISTANCE = 60;

interface Props {
  items: GalleryItem[];
  index: number;
  onIndexChange: (index: number) => void;
  onClose: () => void;
}

export function ImageGallery({ items, index, onIndexChange, onClose }: Props) {
  const item = items[index];
  const { getUser } = useUserCache();
  const [zoomedId, setZoomedId] = useState<string | null>(null);
  const swipeStart = useRef<number | null>(null);
  const zoomed = zoomedId === item?.attachment.id;
  const hasPrevious = index > 0;
  const hasNext = index < items.length - 1;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'ArrowLeft' && index > 0) onIndexChange(index - 1);
      if (event.key === 'ArrowRight' && index < items.length - 1) onIndexChange(index + 1);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [index, items.length, onIndexChange]);

  if (!item) return null;
  const { attachment } = item;
  const sender = item.userId ? getUser(item.userId) : undefined;
  const control =
    'w-8 h-8 rounded-md flex items-center justify-center text-white/70 hover:text-white hover:bg-white/10 transition';
  const step =
    'absolute top-1/2 -translate-y-1/2 flex h-10 w-10 items-center justify-center rounded-full bg-black/60 text-white transition hover:bg-black/80';

  return (
    <Modal
      title={attachment.filename}
      onClose={onClose}
      dataQa="image-lightbox"
      backdropClassName="fixed inset-0 bg-overlay/95 flex items-center justify-center z-50 p-2 sm:p-6"
      className="flex h-full w-full max-w-[96vw] flex-col gap-3"
    >
      <div className="flex items-center gap-3">
        {item.userId && (
          <Avatar
            userId={item.userId}
            name={displayNameOf(sender?.display_name)}
            avatarUrl={sender?.avatar_url}
            size="sm"
          />
        )}
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium text-white" title={attachment.filename}>
            {attachment.filename}
          </div>
          <div className="truncate text-xs text-white/60" data-qa="gallery-meta">
            {[
              item.userId ? displayNameOf(sender?.display_name) : null,
              item.createdAt ? formatDateTime(item.createdAt) : null,
            ]
              .filter(Boolean)
              .join(' · ')}
          </div>
        </div>
        {items.length > 1 && (
          <span className="text-xs text-white/60 tabular-nums" data-qa="gallery-counter">
            {index + 1} / {items.length}
          </span>
        )}
        <a
          href={attachment.url}
          target="_blank"
          rel="noopener noreferrer"
          aria-label="Open original"
          title="Open original"
          className={control}
        >
          <ExternalLink className="w-4 h-4" />
        </a>
        <a
          href={attachment.url}
          download={attachment.filename}
          data-qa="lightbox-download"
          aria-label={`Download ${attachment.filename}`}
          className={control}
        >
          <Download className="w-4 h-4" />
        </a>
        <button
          type="button"
          onClick={onClose}
          data-qa="lightbox-close"
          aria-label="Close preview"
          className={control}
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      <div
        className={`relative flex min-h-0 flex-1 items-center justify-center ${zoomed ? 'overflow-auto' : 'overflow-hidden'}`}
        style={{ touchAction: zoomed ? 'pan-x pan-y pinch-zoom' : 'pan-y pinch-zoom' }}
        onPointerDown={(event) => {
          swipeStart.current = event.pointerType === 'mouse' ? null : event.clientX;
        }}
        onPointerUp={(event) => {
          const start = swipeStart.current;
          swipeStart.current = null;
          if (start === null || zoomed) return;
          const moved = event.clientX - start;
          if (moved > SWIPE_DISTANCE && hasPrevious) onIndexChange(index - 1);
          if (moved < -SWIPE_DISTANCE && hasNext) onIndexChange(index + 1);
        }}
      >
        <img
          key={attachment.id}
          src={attachment.url}
          alt={attachment.filename}
          data-qa="gallery-image"
          onClick={() => setZoomedId(zoomed ? null : attachment.id)}
          className={
            zoomed
              ? 'max-w-none cursor-zoom-out'
              : 'max-h-[80dvh] max-w-full cursor-zoom-in rounded-lg object-contain'
          }
        />
        {hasPrevious && (
          <button
            type="button"
            onClick={() => onIndexChange(index - 1)}
            aria-label="Previous image"
            data-qa="gallery-previous"
            className={`${step} left-2`}
          >
            <ChevronLeft className="h-6 w-6" />
          </button>
        )}
        {hasNext && (
          <button
            type="button"
            onClick={() => onIndexChange(index + 1)}
            aria-label="Next image"
            data-qa="gallery-next"
            className={`${step} right-2`}
          >
            <ChevronRight className="h-6 w-6" />
          </button>
        )}
      </div>
    </Modal>
  );
}
