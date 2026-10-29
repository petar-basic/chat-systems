import { Check } from 'lucide-react';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import type { AppPermission } from '@/features/workspace/hooks/useAppDirectory';

interface Props {
  title: string;
  lead: string;
  permissions: AppPermission[];
  confirmLabel: string;
  onConfirm: () => void | Promise<void>;
  onClose: () => void;
}

export function ConsentDialog({ title, lead, permissions, confirmLabel, onConfirm, onClose }: Props) {
  return (
    <ConfirmDialog
      title={title}
      description={lead}
      confirmLabel={confirmLabel}
      tone="primary"
      onConfirm={onConfirm}
      onClose={onClose}
      dataQa="app-consent"
    >
      {permissions.length === 0 ? (
        <p className="text-sm text-muted">Nothing beyond being listed here.</p>
      ) : (
        <ul className="space-y-1.5" data-qa="app-consent-permissions">
          {permissions.map((permission) => (
            <li key={permission.key} className="flex items-start gap-2 text-sm text-fg-soft">
              <Check className="mt-0.5 h-4 w-4 shrink-0 text-success" />
              {permission.text}
            </li>
          ))}
        </ul>
      )}
    </ConfirmDialog>
  );
}
