import { useEffect, useRef } from 'react';
import { useNavigate, type NavigateFunction } from 'react-router';
import { getCurrent, onOpenUrl } from '@tauri-apps/plugin-deep-link';
import { linkInside, placeOf, type AppPlace } from '@/lib/appLinks';
import { inPhoneApp } from '@/lib/phoneApp';
import { logger } from '@/lib/logger';
import { useInstanceStore } from '@/stores/instances';
import { ROUTES } from '@/shared/constants';

function servers(): string[] {
  return useInstanceStore.getState().instances.map((i) => i.url);
}

function open(navigate: NavigateFunction, place: AppPlace) {
  useInstanceStore.getState().setActiveInstance(place.server);
  void navigate(place.path);
}

function followed(event: MouseEvent): HTMLAnchorElement | null {
  if (event.defaultPrevented || event.button !== 0) return null;
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return null;
  const anchor = event.target instanceof Element ? event.target.closest('a[href]') : null;
  return anchor instanceof HTMLAnchorElement && !anchor.hasAttribute('download') ? anchor : null;
}

export function AppLinks() {
  const navigate = useNavigate();
  const go = useRef(navigate);

  useEffect(() => {
    go.current = navigate;
  }, [navigate]);

  useEffect(() => {
    const onClick = (event: MouseEvent) => {
      const anchor = followed(event);
      const place = anchor ? placeOf(anchor.href, servers()) : null;
      if (!place) return;
      event.preventDefault();
      open(go.current, place);
    };
    document.addEventListener('click', onClick);
    return () => document.removeEventListener('click', onClick);
  }, []);

  useEffect(() => {
    if (!inPhoneApp) return;
    let stop: (() => void) | null = null;
    let gone = false;
    const follow = (links: string[] | null) => {
      for (const deepLink of links ?? []) {
        const link = linkInside(deepLink);
        if (!link) continue;
        const place = placeOf(link.href, servers());
        if (place) open(go.current, place);
        else void go.current(ROUTES.addInstance, { state: { server: link.origin } });
      }
    };
    getCurrent()
      .then((links) => {
        if (!gone) follow(links);
      })
      .catch((err: unknown) => logger.error('AppLinks', 'getCurrent', err));
    onOpenUrl(follow)
      .then((unlisten) => {
        if (gone) unlisten();
        else stop = unlisten;
      })
      .catch((err: unknown) => logger.error('AppLinks', 'onOpenUrl', err));
    return () => {
      gone = true;
      stop?.();
    };
  }, []);

  return null;
}
