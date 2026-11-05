import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { X } from 'lucide-react';
import { TextInput } from '@/shared/components/Form/TextInput';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { toast } from '@/shared/components/Toast';
import { useWorkspaceStore } from '@/stores/workspace';
import {
  useNotificationKeywords,
  useSetNotificationKeywords,
} from '@/features/notifications/hooks/useNotificationKeywords';
import { keywordProblem } from '@/lib/keywords';

export function KeywordsPanel() {
  const { t } = useTranslation('notifications');
  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const { data: keywords } = useNotificationKeywords(workspace?.id ?? null, workspace?.instanceUrl);
  const save = useSetNotificationKeywords(workspace?.id ?? '', workspace?.instanceUrl);
  const [draft, setDraft] = useState('');
  const [error, setError] = useState<string | null>(null);
  if (!workspace || !keywords) return null;

  const store = (next: string[]) =>
    save.mutate(next, { onError: () => toast.error(t('keywords.saveFailed')) });

  const add = () => {
    const word = draft.trim();
    if (!word) return;
    const problem = keywordProblem(word, keywords);
    setError(problem && t(`keywords.problems.${problem.kind}`, problem));
    if (problem) return;
    setDraft('');
    store([...keywords, word]);
  };

  return (
    <div data-qa="notification-keywords">
      <TextInput
        label={t('keywords.label')}
        hint={t('keywords.hint')}
        placeholder={t('keywords.placeholder')}
        value={draft}
        maxLength={64}
        error={error}
        onChange={(e) => {
          setDraft(e.target.value);
          setError(null);
        }}
        onKeyDown={(e) => {
          if (e.key !== 'Enter' && e.key !== ',') return;
          e.preventDefault();
          add();
        }}
        dataQa="notification-keyword-input"
      />
      {keywords.length > 0 && (
        <ul aria-label={t('keywords.label')} className="mt-2 flex flex-wrap gap-1.5">
          {keywords.map((keyword) => (
            <li
              key={keyword}
              data-qa="notification-keyword"
              className="inline-flex items-center gap-0.5 rounded-full bg-raised py-0.5 pl-2.5 pr-0.5 text-sm text-fg-dim"
            >
              {keyword}
              <IconButton
                label={t('keywords.remove', { keyword })}
                icon={<X className="h-3 w-3" />}
                className="h-5 w-5 rounded-full"
                onClick={() => store(keywords.filter((k) => k !== keyword))}
              />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
