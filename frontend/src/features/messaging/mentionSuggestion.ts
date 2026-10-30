import { ReactRenderer } from '@tiptap/react';
import { PluginKey } from '@tiptap/pm/state';
import type { SuggestionOptions } from '@tiptap/suggestion';
import { MentionDropdown, type MentionDropdownHandle, type MentionItem } from './MentionDropdown';
import { placeSuggestion } from './suggestionPosition';

export const mentionSuggestionPluginKey = new PluginKey('mentionSuggestion');

export function createMentionSuggestion(
  getItems: (query: string) => MentionItem[],
): Partial<SuggestionOptions<MentionItem>> {
  return {
    char: '@',
    pluginKey: mentionSuggestionPluginKey,
    items: ({ query }) => getItems(query),

    render: () => {
      let renderer: ReactRenderer<MentionDropdownHandle>;
      let container: HTMLDivElement;

      return {
        onStart(props) {
          container = document.createElement('div');
          container.style.cssText = 'position:fixed;z-index:9999;pointer-events:auto';
          document.body.appendChild(container);

          renderer = new ReactRenderer(MentionDropdown, {
            props,
            editor: props.editor,
          });
          container.appendChild(renderer.element);
          placeSuggestion(container, props.clientRect?.(), 200);
        },

        onUpdate(props) {
          renderer.updateProps(props);
          placeSuggestion(container, props.clientRect?.(), 200);
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
  };
}
