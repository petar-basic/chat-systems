const EMOTICONS: Record<string, string> = {
  '<3': '❤️',
  '</3': '💔',
  ':)': '🙂',
  ':-)': '🙂',
  ':(': '🙁',
  ':-(': '🙁',
  ':D': '😃',
  ':-D': '😃',
  ';)': '😉',
  ';-)': '😉',
  ';(': '😢',
  ":'(": '😢',
  ':P': '😛',
  ':-P': '😛',
  ':p': '😛',
  ':-p': '😛',
  ':O': '😮',
  ':-O': '😮',
  ':o': '😮',
  ':-o': '😮',
  ':|': '😐',
  ':-|': '😐',
  ':*': '😘',
  ':-*': '😘',
  ':@': '😠',
  '>:(': '😠',
  '8)': '😎',
  '8-)': '😎',
  xD: '😆',
  XD: '😆',
};

function escapeForRegex(literal: string): string {
  return literal.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

const ALTERNATION = Object.keys(EMOTICONS)
  .sort((a, b) => b.length - a.length)
  .map(escapeForRegex)
  .join('|');

export const EMOTICON_RULE = new RegExp(`(^|\\s)(${ALTERNATION})$`);

export function emoticonToEmoji(emoticon: string): string | undefined {
  return EMOTICONS[emoticon];
}
