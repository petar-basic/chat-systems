import { ROUTES } from '@/shared/constants';

export interface AppPlace {
  server: string;
  path: string;
}

function parse(href: string): URL | null {
  try {
    return new URL(href);
  } catch {
    return null;
  }
}

export function placeOf(href: string, servers: string[]): AppPlace | null {
  const link = parse(href);
  if (!link || (link.protocol !== 'https:' && link.protocol !== 'http:')) return null;
  for (const server of servers) {
    const base = parse(server);
    if (!base || base.origin !== link.origin) continue;
    const prefix = base.pathname.replace(/\/$/, '');
    if (link.pathname !== prefix && !link.pathname.startsWith(`${prefix}/`)) continue;
    const rest = link.pathname.slice(prefix.length);
    if (rest === '' || rest === '/') return { server, path: ROUTES.app };
    if (rest !== ROUTES.app && !rest.startsWith(`${ROUTES.app}/`)) continue;
    return { server, path: `${rest}${link.search}` };
  }
  return null;
}

export function linkInside(deepLink: string): URL | null {
  const deep = parse(deepLink);
  if (!deep || deep.protocol !== 'chat-systems:' || deep.host !== 'open') return null;
  const link = parse(deep.searchParams.get('url') ?? '');
  return link && (link.protocol === 'https:' || link.protocol === 'http:') ? link : null;
}
