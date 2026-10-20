import { Download, X } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import type { ParsedAttachment } from '@/lib/attachments';

interface ImageLightboxProps extends Pick<ParsedAttachment, 'filename' | 'url'> {
  onClose: () => void;
}

export function ImageLightbox({ filename, url, onClose }: ImageLightboxProps) {
  return (
    <Modal
      title={filename}
      onClose={onClose}
      dataQa="image-lightbox"
      backdropClassName="fixed inset-0 bg-overlay/90 flex items-center justify-center z-50 p-4 sm:p-8"
      className="flex flex-col gap-3 max-w-[92vw]"
    >
      <div className="flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate text-sm text-white/80" title={filename}>
          {filename}
        </span>
        <a
          href={url}
          download={filename}
          data-qa="lightbox-download"
          aria-label={`Download ${filename}`}
          className="w-8 h-8 rounded-md flex items-center justify-center text-white/70 hover:text-white hover:bg-white/10 transition"
        >
          <Download className="w-4 h-4" />
        </a>
        <button
          type="button"
          onClick={onClose}
          data-qa="lightbox-close"
          aria-label="Close preview"
          className="w-8 h-8 rounded-md flex items-center justify-center text-white/70 hover:text-white hover:bg-white/10 transition"
        >
          <X className="w-4 h-4" />
        </button>
      </div>
      <img src={url} alt={filename} className="max-h-[78dvh] max-w-full rounded-lg object-contain" />
    </Modal>
  );
}
