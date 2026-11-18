import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2 } from 'lucide-react';
import { useBotAppUnfurlDomains, useSaveBotAppUnfurlDomains } from '@/features/apps';
import type { BotApp } from '@/features/workspace/hooks/useBotApps';
import { Button } from '@/shared/components/Button/Button';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { TextInput } from '@/shared/components/Form/TextInput';
import { toUserMessage } from '@/lib/errors';

const MAX_DOMAINS = 5;

interface Props {
  app: BotApp;
}

export function BotAppUnfurlDomains({ app }: Props) {
  const listed = useBotAppUnfurlDomains(app.id);
  if (!listed.data) return null;
  const allowed = app.scopes.includes('links:read') && app.scopes.includes('links:write');
  return <DomainsForm key={listed.dataUpdatedAt} appId={app.id} initial={listed.data} allowed={allowed} />;
}

interface FormProps {
  appId: string;
  initial: string[];
  allowed: boolean;
}

function DomainsForm({ appId, initial, allowed }: FormProps) {
  const { t } = useTranslation('workspace');
  const save = useSaveBotAppUnfurlDomains(appId);
  const [domains, setDomains] = useState<string[]>(initial);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    save.mutate(domains.map((domain) => domain.trim()).filter(Boolean));
  };

  return (
    <form
      onSubmit={submit}
      className="space-y-3 border-t border-line/50 pt-3"
      data-qa="bot-app-unfurl-domains"
    >
      <div>
        <h4 className="text-xs font-medium text-fg-dim">{t('settings.apps.unfurls.legend')}</h4>
        <p className="text-xs text-muted">{t('settings.apps.unfurls.hint', { max: MAX_DOMAINS })}</p>
        {!allowed && (
          <p className="mt-1 text-xs text-warning" data-qa="bot-app-unfurl-needs-scopes">
            {t('settings.apps.unfurls.needsScopes')}
          </p>
        )}
      </div>
      {domains.map((domain, index) => (
        <div key={index} className="flex items-end gap-2" data-qa="bot-app-unfurl-domain">
          <div className="min-w-0 flex-1">
            <TextInput
              label={t('settings.apps.unfurls.domain', { number: index + 1 })}
              value={domain}
              onChange={(e) =>
                setDomains((current) => current.map((d, i) => (i === index ? e.target.value : d)))
              }
              placeholder="tracker.example.com"
              dataQa="bot-app-unfurl-domain-input"
            />
          </div>
          <IconButton
            label={t('settings.apps.unfurls.remove', { domain: domain || index + 1 })}
            icon={<Trash2 className="h-4 w-4" />}
            tone="danger"
            onClick={() => setDomains((current) => current.filter((_, i) => i !== index))}
            data-qa="bot-app-unfurl-domain-remove"
          />
        </div>
      ))}
      <div className="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          size="sm"
          onClick={() => setDomains((current) => [...current, ''])}
          disabled={!allowed || domains.length >= MAX_DOMAINS}
          data-qa="bot-app-unfurl-domain-add"
        >
          <Plus className="h-3.5 w-3.5" /> {t('settings.apps.unfurls.add')}
        </Button>
        <Button
          type="submit"
          variant="primary"
          size="sm"
          loading={save.isPending}
          data-qa="bot-app-unfurl-domains-save"
        >
          {t('settings.apps.unfurls.save')}
        </Button>
        {save.isSuccess && !save.isPending && (
          <span className="text-xs text-muted" data-qa="bot-app-unfurl-domains-saved">
            {t('settings.apps.unfurls.saved')}
          </span>
        )}
      </div>
      {save.error && (
        <p role="alert" className="text-sm text-danger">
          {toUserMessage(save.error, t('settings.apps.unfurls.failed'))}
        </p>
      )}
    </form>
  );
}
