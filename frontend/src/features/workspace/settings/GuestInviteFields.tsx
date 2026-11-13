import { useTranslation } from 'react-i18next';
import { Select } from '@/shared/components/Form/Select';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { TextInput } from '@/shared/components/Form/TextInput';
import { useWorkspaceChannels } from '@/features/workspace/hooks/useWorkspaces';
import { useBrowsableChannels } from '@/features/channel';
import { guestChannelChoices, toggleChannel, tomorrow, type GuestInvite } from './guestTerms';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  value: GuestInvite;
  onChange: (value: GuestInvite) => void;
}

export function GuestInviteFields({ workspaceId, instanceUrl, value, onChange }: Props) {
  const { t } = useTranslation('workspace');
  const { data: mine = [] } = useWorkspaceChannels(workspaceId, instanceUrl);
  const { data: browsable = [] } = useBrowsableChannels(workspaceId, instanceUrl);
  const choices = guestChannelChoices(mine, browsable);

  return (
    <div className="space-y-2" data-qa="guest-invite-fields">
      <Select
        label={t('guest.kind')}
        value={value.kind}
        onChange={(e) => {
          const kind = e.target.value === 'single' ? 'single' : 'multi';
          onChange({
            ...value,
            kind,
            channelIds: kind === 'single' ? value.channelIds.slice(0, 1) : value.channelIds,
          });
        }}
        dataQa="guest-invite-kind"
      >
        <option value="single">{t('guest.single')}</option>
        <option value="multi">{t('guest.multi')}</option>
      </Select>
      <fieldset className="space-y-1.5">
        <legend className="text-xs text-muted mb-1">
          {value.kind === 'single' ? t('guest.channelOne') : t('guest.channelsSome')}
        </legend>
        <div className="max-h-40 overflow-y-auto space-y-1.5 px-1" data-qa="guest-invite-channels">
          {choices.map((ch) => (
            <Checkbox
              key={ch.id}
              label={`#${ch.name}`}
              checked={value.channelIds.includes(ch.id)}
              onChange={(on) => onChange(toggleChannel(value, ch.id, on))}
              dataQa={`guest-invite-channel-${ch.name}`}
            />
          ))}
        </div>
      </fieldset>
      <TextInput
        label={t('guest.until')}
        optional
        type="date"
        min={tomorrow()}
        value={value.until}
        onChange={(e) => onChange({ ...value, until: e.target.value })}
        dataQa="guest-invite-until"
      />
    </div>
  );
}
