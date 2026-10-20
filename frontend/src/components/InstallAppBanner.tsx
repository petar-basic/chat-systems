import { Download, Share, X } from 'lucide-react';
import { usePwaInstall } from '@/stores/pwaInstall';

export default function InstallAppBanner() {
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
        <span className="font-medium text-fg">Install Chat Systems</span>
        <span className="text-muted">
          {offer === 'ios' ? (
            <>
              {' — tap '}
              <Share className="inline w-3.5 h-3.5 -mt-0.5" aria-label="Share" />
              {' then “Add to Home Screen”'}
            </>
          ) : (
            ' — its own window, and notifications when it is closed'
          )}
        </span>
      </div>
      {offer === 'prompt' && (
        <button
          type="button"
          onClick={() => void install()}
          data-qa="install-banner-install"
          className="px-3 py-1.5 text-sm font-medium bg-purple-600 hover:bg-purple-500 text-white rounded-lg transition cursor-pointer shrink-0"
        >
          Install
        </button>
      )}
      <button
        type="button"
        onClick={dismiss}
        aria-label="Dismiss install prompt"
        data-qa="install-banner-dismiss"
        className="p-1 text-muted hover:text-fg transition cursor-pointer shrink-0"
      >
        <X className="w-4 h-4" />
      </button>
    </div>
  );
}
