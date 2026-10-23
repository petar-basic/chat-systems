export interface StatusDraft {
  emoji: string;
  text: string;
  minutes: number | null;
}

export function sameStatus(a: StatusDraft, b: StatusDraft) {
  return a.emoji.trim() === b.emoji.trim() && a.text.trim() === b.text.trim() && a.minutes === b.minutes;
}
