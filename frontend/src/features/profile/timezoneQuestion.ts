const ASKED_KEY = 'chat_timezone_asked';

export function askedAbout(): string | null {
  try {
    return localStorage.getItem(ASKED_KEY);
  } catch {
    return null;
  }
}

export function rememberAsked(zone: string) {
  try {
    localStorage.setItem(ASKED_KEY, zone);
  } catch {
    return;
  }
}
