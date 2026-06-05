import { Extension } from '@tiptap/core';
import Suggestion from '@tiptap/suggestion';
import { PluginKey } from '@tiptap/pm/state';
import { ReactRenderer } from '@tiptap/react';
import { searchEmojis, type EmojiSuggestionItem } from '@/lib/emojiData';
import { EmojiSuggestionDropdown, type EmojiSuggestionHandle } from './EmojiSuggestionDropdown';

export const emojiSuggestionPluginKey = new PluginKey('emojiSuggestion');

export const EmojiSuggestion = Extension.create({
  name: 'emojiSuggestion',
  addProseMirrorPlugins() {
    return [
      Suggestion<EmojiSuggestionItem>({
        editor: this.editor,
        char: ':',
        pluginKey: emojiSuggestionPluginKey,
        allowSpaces: false,
        command: ({ editor, range, props }) => {
          editor.chain().focus().insertContentAt(range, `${props.native} `).run();
        },
        items: ({ query }) => (query.length >= 2 ? searchEmojis(query) : Promise.resolve([])),

        render: () => {
          let renderer: ReactRenderer<EmojiSuggestionHandle>;
          let container: HTMLDivElement;

          return {
            onStart(props) {
              container = document.createElement('div');
              container.style.cssText = 'position:fixed;z-index:9999;pointer-events:auto';
              document.body.appendChild(container);

              renderer = new ReactRenderer(EmojiSuggestionDropdown, {
                props,
                editor: props.editor,
              });
              container.appendChild(renderer.element);
              positionContainer(container, props.clientRect?.());
            },

            onUpdate(props) {
              renderer.updateProps(props);
              positionContainer(container, props.clientRect?.());
            },

            onKeyDown(props) {
              if (props.event.key === 'Escape') {
                container.remove();
                renderer.destroy();
                return true;
              }
              return renderer.ref?.onKeyDown(props) ?? false;
            },

            onExit() {
              container.remove();
              renderer.destroy();
            },
          };
        },
      }),
    ];
  },
});

function positionContainer(el: HTMLDivElement, rect?: DOMRect | null) {
  if (!rect) return;
  const spaceBelow = window.innerHeight - rect.bottom;
  if (spaceBelow < 260) {
    el.style.top = '';
    el.style.bottom = `${window.innerHeight - rect.top + 4}px`;
  } else {
    el.style.bottom = '';
    el.style.top = `${rect.bottom + 4}px`;
  }
  el.style.left = `${rect.left}px`;
}
