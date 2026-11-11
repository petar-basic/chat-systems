import { afterEach, describe, expect, it } from 'vitest';
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import {
  EmojiShortcodes,
  convertTrailingEmoticon,
  loadShortcodes,
  shortcodeToEmoji,
} from './emojiShortcodes';

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

  it('leaves Enter after an emoticon to send the message', () => {
    type('thanks :)');
    const { view } = editor!;
    const inputRules = view.state.plugins.find(
      (plugin) => (plugin.spec as { isInputRules?: boolean }).isInputRules,
    );
    const handled = inputRules?.props.handleKeyDown?.call(
      inputRules,
      view,
      new KeyboardEvent('keydown', { key: 'Enter' }),
    );
    expect(inputRules).toBeDefined();
    expect(handled).toBe(false);
    expect(editor!.state.doc.textContent).toBe('thanks :)');
  });

  it('converts an emoticon that ends the message when it is sent', () => {
    type('thanks :)');
    convertTrailingEmoticon(editor!);
    expect(editor!.state.doc.textContent).toBe('thanks 🙂');
  });

  it('converts an emoticon that is the whole message', () => {
    type(':D');
    convertTrailingEmoticon(editor!);
    expect(editor!.state.doc.textContent).toBe('😃');
  });

  it('leaves an emoticon glued to a word, or in code, as typed', () => {
    type('see https://x.com:D');
    convertTrailingEmoticon(editor!);
    expect(editor!.state.doc.textContent).toBe('see https://x.com:D');

    editor!.commands.setContent('<p>run <code>echo :)</code></p>');
    convertTrailingEmoticon(editor!);
    expect(editor!.state.doc.textContent).toBe('run echo :)');

    editor!.commands.setContent('<pre><code>x = :)</code></pre>');
    convertTrailingEmoticon(editor!);
    expect(editor!.state.doc.textContent).toBe('x = :)');
  });
});
