import { useTranslation } from 'react-i18next';
import { AlertTriangle } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { useServerSupportStore, type ServerProblem } from '@/stores/serverSupport';

function serverName(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

function Problem({ url, problem }: { url: string; problem: ServerProblem }) {
  const { t } = useTranslation('common');
  const server = serverName(url);

  return (
    <div
      role="status"
      data-qa="server-support-banner"
      className="flex items-center gap-3 px-4 py-2.5 border-b border-amber-500/20 bg-amber-500/10"
    >
      <AlertTriangle className="w-4 h-4 text-warning shrink-0" />
      {problem.kind === 'needsNewerApp' ? (
        <>
          <div className="flex-1 min-w-0 text-sm">
            <span className="font-medium text-fg">{t('serverSupport.needsNewerApp', { server })}</span>
            <span className="text-muted">
              {t('serverSupport.needsNewerAppHint', { version: problem.minVersion })}
            </span>
          </div>
          <Button
            variant="primary"
            className="shrink-0"
            onClick={() => window.location.reload()}
            data-qa="server-support-reload"
          >
            {t('newVersion.reload')}
          </Button>
        </>
      ) : (
        <div className="flex-1 min-w-0 text-sm">
          <span className="font-medium text-fg">{t('serverSupport.serverTooOld', { server })}</span>
          <span className="text-muted">{t('serverSupport.serverTooOldHint')}</span>
        </div>
      )}
    </div>
  );
}

export default function ServerSupportBanner() {
  const problems = useServerSupportStore((s) => s.problems);

  return (
    <>
      {Object.entries(problems).map(([url, problem]) => (
        <Problem key={url} url={url} problem={problem} />
      ))}
    </>
  );
}
