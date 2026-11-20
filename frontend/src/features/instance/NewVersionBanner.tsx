import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { runningBuild, watchForNewBuild } from '@/lib/appVersion';

export default function NewVersionBanner() {
  const { t } = useTranslation('common');
  const [available, setAvailable] = useState(false);

  useEffect(() => watchForNewBuild(runningBuild, () => setAvailable(true)), []);

  if (!available) return null;

  return (
    <div
      role="status"
      data-qa="new-version-banner"
      className="flex items-center gap-3 px-4 py-2.5 border-b border-line bg-purple-600/10"
    >
      <RefreshCw className="w-4 h-4 text-accent shrink-0" />
      <div className="flex-1 min-w-0 text-sm">
        <span className="font-medium text-fg">{t('newVersion.title')}</span>
        <span className="text-muted">{t('newVersion.hint')}</span>
      </div>
      <Button
        variant="primary"
        className="shrink-0"
        onClick={() => window.location.reload()}
        data-qa="new-version-reload"
      >
        {t('newVersion.reload')}
      </Button>
    </div>
  );
}
