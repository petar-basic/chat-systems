import { useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { useInstanceSettings, useUpdateInstanceSettings } from '@/features/instance';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Button } from '@/shared/components/Button/Button';
import { toUserMessage } from '@/lib/errors';
import { QUERY_KEYS } from '@/shared/constants';

interface Props {
  instanceUrl: string | null;
}

export function GiphyKeySetting({ instanceUrl }: Props) {
  const { t } = useTranslation('admin');
  const { data } = useInstanceSettings(instanceUrl);
  const update = useUpdateInstanceSettings(instanceUrl);
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState<string | null>(null);
  if (!data) return null;

  const saved = data.giphy_api_key ?? '';
  const value = draft ?? saved;
  const save = (key: string) =>
    update.mutate(
      { giphy_api_key: key },
      {
        onSuccess: () => {
          setDraft(null);
          void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.gifSettingsAll() });
          void queryClient.invalidateQueries({ queryKey: QUERY_KEYS.slashCommandsAll() });
        },
      },
    );

  return (
    <form
      className="space-y-2 border-t border-line pt-6"
      data-qa="giphy-key-setting"
      onSubmit={(e) => {
        e.preventDefault();
        save(value);
      }}
    >
      <TextInput
        label={t('giphy.label')}
        hint={t('giphy.hint')}
        value={value}
        onChange={(e) => setDraft(e.target.value)}
        autoComplete="off"
        spellCheck={false}
        maxLength={128}
        dataQa="giphy-key-input"
      />
      <div className="flex gap-2">
        <Button
          type="submit"
          variant="primary"
          size="sm"
          disabled={value.trim() === saved}
          loading={update.isPending}
          data-qa="giphy-key-save"
        >
          {t('giphy.save')}
        </Button>
        {saved && (
          <Button
            variant="ghost"
            size="sm"
            disabled={update.isPending}
            onClick={() => save('')}
            data-qa="giphy-key-remove"
          >
            {t('giphy.remove')}
          </Button>
        )}
      </div>
      {update.isError && (
        <p className="text-sm text-danger" role="alert">
          {toUserMessage(update.error, t('saveFailed'))}
        </p>
      )}
    </form>
  );
}
