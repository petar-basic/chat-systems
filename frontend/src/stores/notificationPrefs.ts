import { create } from 'zustand';

const STORAGE_KEY = 'chat_notif_prefs';

interface StoredPrefs {
  soundEnabled: boolean;
  desktopEnabled: boolean;
}

function load(): StoredPrefs {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { soundEnabled: true, desktopEnabled: true };
    const parsed = JSON.parse(raw) as Partial<StoredPrefs>;
    return {
      soundEnabled: parsed.soundEnabled ?? true,
      desktopEnabled: parsed.desktopEnabled ?? true,
    };
  } catch {
    return { soundEnabled: true, desktopEnabled: true };
  }
}

interface NotificationPrefsState extends StoredPrefs {
  toggleSound: () => void;
  toggleDesktop: () => void;
}

export const useNotificationPrefs = create<NotificationPrefsState>((set, get) => {
  const persist = () => {
    const { soundEnabled, desktopEnabled } = get();
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ soundEnabled, desktopEnabled }));
  };
  return {
    ...load(),
    toggleSound: () => {
      set((s) => ({ soundEnabled: !s.soundEnabled }));
      persist();
    },
    toggleDesktop: () => {
      set((s) => ({ desktopEnabled: !s.desktopEnabled }));
      persist();
    },
  };
});
