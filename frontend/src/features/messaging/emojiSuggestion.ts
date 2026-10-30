import { Extension } from '@tiptap/core';
import Suggestion from '@tiptap/suggestion';
import { PluginKey } from '@tiptap/pm/state';
import { ReactRenderer } from '@tiptap/react';
import { searchEmojis, type EmojiSuggestionItem } from '@/lib/emojiData';
import { matchCustomEmoji } from '@/lib/customEmoji';
import { useCustomEmojiStore } from '@/stores/customEmoji';
import { EmojiSuggestionDropdown, type EmojiSuggestionHandle } from './EmojiSuggestionDropdown';
import { placeSuggestion } from './suggestionPosition';

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
          editor
            .chain()
            .focus()
            .insertContentAt(range, props.src ? `:${props.id}: ` : `${props.native} `)
            .run();
        },
        items: async ({ query }) => {
          if (query.length < 2) return [];
          const custom = matchCustomEmoji(query, useCustomEmojiStore.getState().byName).map((emoji) => ({
            id: emoji.name,
            name: emoji.name,
            native: '',
            src: emoji.url,
          }));
          return [...custom, ...(await searchEmojis(query))].slice(0, 8);
        },

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
              placeSuggestion(container, props.clientRect?.(), 260);
            },

            onUpdate(props) {
              renderer.updateProps(props);
              placeSuggestion(container, props.clientRect?.(), 260);
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
