import { useState, type ReactNode } from 'react';
import { ChevronRight } from 'lucide-react';
import { useDisplayStore } from '@/stores/display';

interface Props {
  label: string;
  dataQa: string;
  children: ReactNode;
}

export function CollapsedPreview({ label, dataQa, children }: Props) {
  const collapsed = useDisplayStore((s) => s.previews === 'collapsed');
  const [open, setOpen] = useState(false);
  if (!collapsed || open) return children;
  return (
    <button
      type="button"
      onClick={() => setOpen(true)}
      aria-expanded={false}
      data-qa={dataQa}
      className="mt-1 inline-flex items-center gap-1 rounded text-xs text-muted hover:text-fg transition cursor-pointer"
    >
      <ChevronRight className="w-3.5 h-3.5" />
      {label}
    </button>
  );
}
