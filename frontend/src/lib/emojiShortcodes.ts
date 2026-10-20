import { Extension, InputRule } from '@tiptap/core';
import type { EditorState } from '@tiptap/pm/state';

import { EMOTICON_RULE, emoticonToEmoji } from './emoticons';

interface ShortcodeData {
  emojis: Record<string, { skins: { native: string }[] }>;
  aliases: Record<string, string>;
}

let table: Map<string, string> | null = null;
let loading: Promise<Map<string, string>> | null = null;

export function loadShortcodes(): Promise<Map<string, string>> {
  if (!loading) {
    loading = import('@emoji-mart/data').then((mod) => {
      const data = mod.default as unknown as ShortcodeData;
      const built = new Map<string, string>();
      for (const [id, emoji] of Object.entries(data.emojis)) {
        const native = emoji.skins[0]?.native;
        if (native) built.set(id, native);
      }
      for (const [alias, id] of Object.entries(data.aliases)) {
        const native = built.get(id);
        if (native) built.set(alias, native);
      }
      table = built;
      return built;
    });
  }
  return loading;
}

export function shortcodeToEmoji(shortcode: string): string | undefined {
  return table?.get(shortcode.toLowerCase());
}

function isVerbatim(state: EditorState, pos: number): boolean {
  const $pos = state.doc.resolve(pos);
  if ($pos.parent.type.spec.code) return true;
  return !!state.schema.marks.code?.isInSet($pos.marks());
}

export const EmojiShortcodes = Extension.create({
  name: 'emojiShortcodes',
  onCreate() {
    void loadShortcodes();
  },
  addInputRules() {
    return [
      new InputRule({
        find: /:([a-zA-Z0-9_+-]+):$/,
        handler: ({ state, range, match }) => {
          if (isVerbatim(state, range.from)) return;
          const emoji = shortcodeToEmoji(match[1]);
          if (emoji) {
            state.tr.insertText(emoji, range.from, range.to);
          }
        },
      }),
      new InputRule({
        find: EMOTICON_RULE,
        handler: ({ state, range, match }) => {
          if (isVerbatim(state, range.from)) return;
          const emoji = emoticonToEmoji(match[2]);
          if (emoji) {
            state.tr.insertText(`${match[1]}${emoji}`, range.from, range.to);
          }
        },
      }),
    ];
  },
});
