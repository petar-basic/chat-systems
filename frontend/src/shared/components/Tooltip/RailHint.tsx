import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { desktopShell, type DesktopRailHint } from '@/lib/desktopShell';

export function RailHint() {
  const [hint, setHint] = useState<DesktopRailHint | null>(null);

  useEffect(
    () =>
      desktopShell()?.onCommand((command) => {
        if (command.kind === 'rail-hint') setHint(command.hint);
      }),
    [],
  );

  if (!hint) return null;

  return createPortal(
    <div
      role="tooltip"
      data-qa="rail-hint"
      style={{ top: hint.y }}
      className="pointer-events-none fixed left-1.5 z-80 -translate-y-1/2"
    >
      <div
        key={hint.text}
        data-side="right"
        className={`max-w-64 rounded-md border border-tooltip-line bg-tooltip px-2 py-1 text-xs font-medium leading-snug text-tooltip-fg shadow-lg break-words ${
          hint.animate ? 'tooltip-enter' : ''
        }`}
      >
        {hint.text}
      </div>
    </div>,
    document.body,
  );
}
