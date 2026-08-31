import { useCallback, useRef, useState, type ReactNode } from 'react';
import { ImagePlus } from 'lucide-react';
import { transferFiles } from '@/lib/fileUploads';

interface Props {
  onFiles: (files: File[]) => void;
  disabled?: boolean;
  className?: string;
  children: ReactNode;
}

export function FileDropZone({ onFiles, disabled = false, className = '', children }: Props) {
  const [dragging, setDragging] = useState(false);
  const depth = useRef(0);

  const reset = useCallback(() => {
    depth.current = 0;
    setDragging(false);
  }, []);

  if (disabled) return <div className={className}>{children}</div>;

  return (
    <div
      className={`relative ${className}`}
      onDragEnter={(e) => {
        if (!e.dataTransfer.types.includes('Files')) return;
        depth.current += 1;
        setDragging(true);
      }}
      onDragOver={(e) => {
        if (!e.dataTransfer.types.includes('Files')) return;
        e.preventDefault();
        e.dataTransfer.dropEffect = 'copy';
      }}
      onDragLeave={() => {
        depth.current -= 1;
        if (depth.current <= 0) reset();
      }}
      onDrop={(e) => {
        reset();
        if (e.defaultPrevented) return;
        const files = transferFiles(e.dataTransfer);
        if (files.length === 0) return;
        e.preventDefault();
        onFiles(files);
      }}
    >
      {children}
      {dragging && (
        <div
          data-qa="file-drop-overlay"
          className="absolute inset-2 z-30 flex flex-col items-center justify-center gap-2 rounded-2xl border-2 border-dashed border-purple-500/70 bg-app/85 text-sm text-fg-dim pointer-events-none"
        >
          <ImagePlus className="w-6 h-6 text-purple-400" />
          Drop to upload
        </div>
      )}
    </div>
  );
}
