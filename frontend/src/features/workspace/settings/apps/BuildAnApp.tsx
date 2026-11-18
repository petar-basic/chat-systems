import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { toUserMessage } from '@/lib/errors';
import { useBotAppActions, type BotAppWithToken, type BotScope } from '@/features/workspace/hooks/useBotApps';
import type { AppPermission } from '@/features/workspace/hooks/useAppDirectory';
import { ConsentDialog } from './ConsentDialog';
import { TokenReveal } from './TokenReveal';

interface Props {
  workspaceId: string;
  scopes: AppPermission[];
}

export function BuildAnApp({ workspaceId, scopes: offered }: Props) {
  const { t } = useTranslation('workspace');
  const { create } = useBotAppActions(workspaceId);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [scopes, setScopes] = useState<BotScope[]>(['chat:write']);
  const [consenting, setConsenting] = useState(false);
  const [minted, setMinted] = useState<BotAppWithToken | null>(null);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (name.trim()) setConsenting(true);
  };

  const make = async () => {
    const app = await create.mutateAsync({ name: name.trim(), description: description.trim(), scopes });
    setMinted(app);
    setName('');
    setDescription('');
    setScopes(['chat:write']);
  };

  const toggle = (scope: BotScope, on: boolean) =>
    setScopes((current) => (on ? [...current, scope] : current.filter((s) => s !== scope)));

  return (
    <section className="space-y-3" data-qa="build-an-app">
      <p className="text-xs text-muted">{t('settings.apps.make.intro')}</p>
      {minted && <TokenReveal name={minted.name} token={minted.token} onDismiss={() => setMinted(null)} />}
      <form
        onSubmit={submit}
        className="space-y-3 rounded-lg border border-line/50 p-3"
        data-qa="bot-app-form"
      >
        <TextInput
          label={t('settings.apps.make.name')}
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder={t('settings.apps.make.namePlaceholder')}
          dataQa="bot-app-name"
        />
        <TextInput
          label={t('settings.apps.make.description')}
          optional
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          dataQa="bot-app-description"
        />
        <fieldset className="space-y-2">
          <legend className="mb-1 text-xs font-medium text-fg-dim">{t('settings.apps.make.scopes')}</legend>
          {offered.map(({ key, text }) => (
            <Checkbox
              key={key}
              label={key}
              description={text}
              checked={scopes.includes(key as BotScope)}
              onChange={(on) => toggle(key as BotScope, on)}
              dataQa={`bot-app-scope-${key}`}
            />
          ))}
        </fieldset>
        <Button
          type="submit"
          variant="primary"
          loading={create.isPending}
          disabled={!name.trim()}
          data-qa="bot-app-create"
        >
          {t('settings.apps.make.create')}
        </Button>
      </form>
      {create.error && (
        <p role="alert" className="text-sm text-danger">
          {toUserMessage(create.error, t('settings.apps.make.failed'))}
        </p>
      )}
      {consenting && (
        <ConsentDialog
          title={t('settings.apps.make.consentTitle', { name: name.trim() })}
          lead={t('settings.apps.make.consentLead')}
          permissions={offered.filter((p) => scopes.includes(p.key as BotScope))}
          confirmLabel={t('settings.apps.make.consentConfirm')}
          onConfirm={make}
          onClose={() => setConsenting(false)}
        />
      )}
    </section>
  );
}
