import { create } from 'zustand';
import { logger } from '@/lib/logger';

const DISMISSED_KEY = 'chat_install_dismissed';

export type InstallOffer = 'prompt' | 'ios' | 'none';

interface BeforeInstallPromptEvent extends Event {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: 'accepted' | 'dismissed' }>;
}

declare global {
  interface Window {
    __installPrompt: BeforeInstallPromptEvent | null;
  }
}

function isStandalone(): boolean {
  if (typeof window === 'undefined') return false;
  const iosStandalone = (window.navigator as Navigator & { standalone?: boolean }).standalone;
  return window.matchMedia?.('(display-mode: standalone)').matches === true || iosStandalone === true;
}

function isIosSafari(): boolean {
  if (typeof navigator === 'undefined') return false;
  const ua = navigator.userAgent;
  const ios =
    /iPad|iPhone|iPod/.test(ua) || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);
  return ios && /Safari/.test(ua) && !/CriOS|FxiOS|EdgiOS/.test(ua);
}

function loadDismissed(): boolean {
  try {
    return localStorage.getItem(DISMISSED_KEY) === '1';
  } catch {
    return false;
  }
}

function currentOffer(): InstallOffer {
  if (isStandalone()) return 'none';
  if (typeof window !== 'undefined' && window.__installPrompt) return 'prompt';
  return isIosSafari() ? 'ios' : 'none';
}

interface PwaInstallState {
  offer: InstallOffer;
  dismissed: boolean;
  install: () => Promise<void>;
  dismiss: () => void;
}

export const usePwaInstall = create<PwaInstallState>((set) => {
  if (typeof window !== 'undefined') {
    window.addEventListener('installpromptchange', () => set({ offer: currentOffer() }));
  }

  return {
    offer: currentOffer(),
    dismissed: loadDismissed(),

    install: async () => {
      const event = window.__installPrompt;
      if (!event) return;
      try {
        await event.prompt();
        const { outcome } = await event.userChoice;
        if (outcome === 'accepted') {
          window.__installPrompt = null;
          set({ offer: 'none' });
        }
      } catch (err) {
        logger.error('usePwaInstall', 'install', err);
      }
    },

    dismiss: () => {
      try {
        localStorage.setItem(DISMISSED_KEY, '1');
      } catch (err) {
        logger.error('usePwaInstall', 'dismiss', err);
      }
      set({ dismissed: true });
    },
  };
});
