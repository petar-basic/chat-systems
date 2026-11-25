import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import i18n from '@/lib/i18n';
import { inAndroidApp } from '@/lib/phoneApp';
import { logger } from '@/lib/logger';
import { useHuddleStore } from '@/stores/huddle';

export function useHuddleInBackground() {
  const talking = useHuddleStore((s) => s.localStream !== null);

  useEffect(() => {
    if (!inAndroidApp || !talking) return;
    invoke('plugin:huddle|begin', {
      channel: i18n.t('huddle:phone.channel'),
      title: i18n.t('huddle:phone.title'),
      text: i18n.t('huddle:phone.text'),
    }).catch((err: unknown) => logger.error('huddle', 'begin', err));
    return () => {
      invoke('plugin:huddle|end').catch((err: unknown) => logger.error('huddle', 'end', err));
    };
  }, [talking]);
}
