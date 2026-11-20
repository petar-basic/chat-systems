import { useTranslation } from 'react-i18next';
import { Download, Share, X } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { Trans } from '@/shared/components/Trans/Trans';
import { usePwaInstall } from '@/stores/pwaInstall';

export default function InstallAppBanner() {
  const { t } = useTranslation('common');
  const offer = usePwaInstall((s) => s.offer);
  const dismissed = usePwaInstall((s) => s.dismissed);
  const install = usePwaInstall((s) => s.install);
  const dismiss = usePwaInstall((s) => s.dismiss);

  if (offer === 'none' || dismissed) return null;

  return (
    <div
      data-qa="install-banner"
      className="flex items-center gap-3 px-4 py-2.5 border-b border-line bg-purple-600/10"
    >
      <Download className="w-4 h-4 text-accent shrink-0" />
      <div className="flex-1 min-w-0 text-sm">
        <span className="font-medium text-fg">{t('installApp.title')}</span>
        <span className="text-muted">
          {offer === 'ios' ? (
            <Trans
              t={t}
              i18nKey="installApp.ios"
              components={{
                share: <Share className="inline w-3.5 h-3.5 -mt-0.5" aria-label={t('installApp.share')} />,
              }}
            />
          ) : (
            t('installApp.window')
          )}
        </span>
      </div>
      {offer === 'prompt' && (
        <Button
          variant="primary"
          className="shrink-0"

          onClick={() => void install()}
          data-qa="install-banner-install"
        >
          {t('installApp.install')}
        </Button>
      )}
      <button
        type="button"
        onClick={dismiss}
        aria-label={t('installApp.dismiss')}
        data-tooltip={t('installApp.dismiss')}
        data-qa="install-banner-dismiss"
        className="p-1 text-muted hover:text-fg transition cursor-pointer shrink-0"
      >
        <X className="w-4 h-4" />
      </button>
    </div>
  );
}
