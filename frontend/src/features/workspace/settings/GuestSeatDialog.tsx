import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { Select } from '@/shared/components/Form/Select';
import { TextInput } from '@/shared/components/Form/TextInput';
import { toUserMessage } from '@/lib/errors';
import type { WorkspaceMember } from '@/stores/workspace';
import { useUpdateGuest } from '@/features/workspace/hooks/useWorkspaces';
import { dayOf, endOfDay, tomorrow, type GuestKind } from './guestTerms';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  member: WorkspaceMember;
  name: string;
  onClose: () => void;
}

export function GuestSeatDialog({ workspaceId, instanceUrl, member, name, onClose }: Props) {
  const { t } = useTranslation(['workspace', 'common']);
  const [kind, setKind] = useState<GuestKind>(member.guest_kind ?? 'multi');
  const [until, setUntil] = useState(member.guest_until ? dayOf(member.guest_until) : '');
  const [error, setError] = useState<string | null>(null);
  const update = useUpdateGuest(workspaceId, instanceUrl);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    try {
      await update.mutateAsync({
        userId: member.user_id,
        guest_kind: kind,
        guest_until: until ? endOfDay(until) : null,
      });
      onClose();
    } catch (err) {
      setError(toUserMessage(err, t('guest.saveFailed')));
    }
  };

  return (
    <Modal
      title={t('guest.editTitle', { name })}
      onClose={onClose}
      onSubmit={handleSubmit}
      dataQa="guest-seat-dialog"
      dismissLabel={t('cancel', { ns: 'common' })}
      actions={
        <Button type="submit" variant="primary" loading={update.isPending} data-qa="guest-seat-save">
          {t('guest.save')}
        </Button>
      }
    >
      <div className="space-y-3">
        <Select
          label={t('guest.kind')}
          value={kind}
          onChange={(e) => setKind(e.target.value === 'single' ? 'single' : 'multi')}
          dataQa="guest-seat-kind"
        >
          <option value="single">{t('guest.single')}</option>
          <option value="multi">{t('guest.multi')}</option>
        </Select>
        <TextInput
          label={t('guest.until')}
          optional
          type="date"
          min={tomorrow()}
          value={until}
          onChange={(e) => setUntil(e.target.value)}
          hint={t('guest.untilHint')}
          dataQa="guest-seat-until"
        />
        {until && (
          <div className="flex justify-end">
            <Button variant="ghost" size="sm" onClick={() => setUntil('')} data-qa="guest-seat-clear">
              {t('guest.clearUntil')}
            </Button>
          </div>
        )}
        {error && (
          <p className="text-xs text-danger" data-qa="guest-seat-error">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
}
