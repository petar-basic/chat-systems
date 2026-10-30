import { Extension } from '@tiptap/core';
import Suggestion from '@tiptap/suggestion';
import { PluginKey } from '@tiptap/pm/state';
import { ReactRenderer } from '@tiptap/react';
import { CommandSuggestionDropdown, type CommandSuggestionHandle } from './CommandSuggestionDropdown';
import { matchSlashCommands, type SlashCommand } from './hooks/useSlashCommands';
import { placeSuggestion } from './suggestionPosition';

declare module '@tiptap/core' {
  interface Storage {
    commandSuggestion: CommandSuggestionStorage;
  }
}

interface CommandSuggestionOptions {
  commands: () => SlashCommand[];
}

interface CommandSuggestionStorage {
  open: boolean;
  exact: boolean;
}

export const commandSuggestionPluginKey = new PluginKey('commandSuggestion');

export const CommandSuggestion = Extension.create<CommandSuggestionOptions, CommandSuggestionStorage>({
  name: 'commandSuggestion',
  addOptions() {
    return { commands: () => [] };
  },
  addStorage() {
    return { open: false, exact: false };
  },
  addProseMirrorPlugins() {
    const storage = this.storage;
    const commands = this.options.commands;
    return [
      Suggestion<SlashCommand>({
        editor: this.editor,
        char: '/',
        startOfLine: true,
        allowSpaces: false,
        pluginKey: commandSuggestionPluginKey,
        items: ({ query }) => matchSlashCommands(commands(), query),
        command: ({ editor, range, props }) => {
          editor.chain().focus().insertContentAt(range, `/${props.command} `).run();
        },
        render: () => {
          let renderer: ReactRenderer<CommandSuggestionHandle>;
          let container: HTMLDivElement;

          const track = (items: SlashCommand[], query: string) => {
            storage.open = items.length > 0;
            storage.exact = items.some((item) => item.command === query.toLowerCase());
          };

          const close = () => {
            storage.open = false;
            storage.exact = false;
            container.remove();
            renderer.destroy();
          };

          return {
            onStart(props) {
              container = document.createElement('div');
              container.style.cssText = 'position:fixed;z-index:9999;pointer-events:auto';
              document.body.appendChild(container);

              renderer = new ReactRenderer(CommandSuggestionDropdown, { props, editor: props.editor });
              container.appendChild(renderer.element);
              track(props.items, props.query);
              placeSuggestion(container, props.clientRect?.(), 300);
            },

            onUpdate(props) {
              renderer.updateProps(props);
              track(props.items, props.query);
              placeSuggestion(container, props.clientRect?.(), 300);
            },

            onKeyDown(props) {
              if (props.event.key === 'Escape') {
                close();
                return true;
              }
              return renderer.ref?.onKeyDown(props) ?? false;
            },

            onExit: close,
          };
        },
      }),
    ];
  },
});
