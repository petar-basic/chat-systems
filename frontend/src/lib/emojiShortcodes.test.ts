import { afterEach, describe, expect, it } from 'vitest';
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import { EmojiShortcodes, loadShortcodes, shortcodeToEmoji } from './emojiShortcodes';

let editor: Editor | null = null;

function type(text: string) {
  editor ??= new Editor({ extensions: [StarterKit, EmojiShortcodes] });
  const { view } = editor;
  for (const char of text) {
    const { from, to } = view.state.selection;
    const handled = view.someProp('handleTextInput', (handle) =>
      handle(view, from, to, char, () => view.state.tr.insertText(char, from, to)),
    );
    if (!handled) view.dispatch(view.state.tr.insertText(char, from, to));
  }
  return editor.state.doc.textContent;
}

describe('emoji shortcodes', () => {
  afterEach(() => {
    editor?.destroy();
    editor = null;
  });

  it('resolves ids and aliases from the emoji-mart data set', async () => {
    await loadShortcodes();
    expect(shortcodeToEmoji('smile')).toBe('😄');
    expect(shortcodeToEmoji('+1')).toBe('👍');
    expect(shortcodeToEmoji('thumbsup')).toBe('👍');
    expect(shortcodeToEmoji('SMILE')).toBe('😄');
    expect(shortcodeToEmoji('not_an_emoji_at_all')).toBeUndefined();
  });

  it('turns an emoticon into emoji only once a space follows it', () => {
    expect(type('ha :D')).toBe('ha :D');
    expect(type(' ')).toBe('ha 😃 ');
  });

  it('leaves an emoticon that grows into a shortcode as typed', () => {
    expect(type(':Dasd: ')).toBe(':Dasd: ');
  });
});
