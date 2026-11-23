import { API_PATH } from './apiVersion';

const DOWNLOADS = `${API_PATH}/files/download/`;

export function mediaUrl(src: string, tokens: Record<string, string>): string {
  let url: URL;
  try {
    url = new URL(src, window.location.href);
  } catch {
    return src;
  }
  const token = tokens[url.origin];
  if (!token || !url.pathname.startsWith(DOWNLOADS)) return src;
  url.searchParams.set('media', token);
  return url.href;
}
