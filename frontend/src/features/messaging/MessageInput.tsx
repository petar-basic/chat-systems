import { useRef, useCallback, useMemo, useEffect, useState } from 'react';
import { Extension } from '@tiptap/core';
import { useEditor, EditorContent } from '@tiptap/react';
import Placeholder from '@tiptap/extension-placeholder';
import Mention from '@tiptap/extension-mention';
import { createEditorExtensions } from '@/lib/tiptapExtensions';
import { createMentionSuggestion, mentionSuggestionPluginKey } from './mentionSuggestion';
import { emojiSuggestionPluginKey } from './emojiSuggestion';
import type { MentionItem } from './MentionDropdown';
import EmojiPicker from './EmojiPicker';
import FormattingToolbar from '@/components/FormattingToolbar';
import type { WorkspaceMember, Channel } from '@/stores/workspace';
import { Paperclip, Send, SmilePlus } from 'lucide-react';
import { MENTION_SUGGESTION_LIMIT, DRAFT_SAVE_DEBOUNCE_MS } from '@/shared/constants';
import { useDraftStore } from '@/stores/drafts';
import { flattenMentions } from '@/lib/mentions';

const MentionNode = Mention.extend({
  addStorage() {
    return {
      markdown: {
        serialize(state: { write: (s: string) => void }, node: { attrs: { label?: string; id: string } }) {
          const label = node.attrs.label ?? node.attrs.id;
          state.write(`@[${label}](${node.attrs.id})`);
        },
        parse: {},
      },
    };
  },
});

interface Props {
  channelName?: string;
  members?: WorkspaceMember[];
  channels?: Channel[];
  isDm?: boolean;
  placeholder?: string;
  draftKey?: string;
  initialContent?: string;
  editing?: boolean;
  onSend: (content: string) => Promise<void>;
  onCancel?: () => void;
  onFileUpload?: (file: File) => Promise<void>;
  onTyping?: () => void;
  uploading?: boolean;
}

export default function MessageInput({
  channelName = '',
  members = [],
  channels = [],
  isDm = false,
  placeholder,
  draftKey,
  initialContent,
  editing = false,
  onSend,
  onCancel,
  onFileUpload,
  onTyping,
  uploading = false,
}: Props) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const draftTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const sendRef = useRef<() => void>(() => {});
  const emojiBtnRef = useRef<HTMLButtonElement>(null);
  const [showEmoji, setShowEmoji] = useState(false);
  const editingRef = useRef(editing);
  editingRef.current = editing;
  const cancelRef = useRef(onCancel);
  cancelRef.current = onCancel;

  const mentionItems = useMemo<MentionItem[]>(
    () => [
      ...members.map((m) => ({ id: m.user_id, label: m.display_name || m.email, type: 'user' as const })),
      ...channels.map((c) => ({ id: c.id, label: c.name, type: 'channel' as const })),
    ],
    [members, channels],
  );
  const mentionItemsRef = useRef<MentionItem[]>(mentionItems);
  useEffect(() => {
    mentionItemsRef.current = mentionItems;
  }, [mentionItems]);

  const mentionExtension = useMemo(
    () =>
      MentionNode.configure({
        HTMLAttributes: { class: 'mention' },
        suggestion: createMentionSuggestion((query) => {
          const q = query.toLowerCase();
          return mentionItemsRef.current
            .filter((item) => item.label.toLowerCase().includes(q))
            .slice(0, MENTION_SUGGESTION_LIMIT);
        }),
      }),
    [],
  );

  const submitExtension = useMemo(
    () =>
      Extension.create({
        name: 'submitOnEnter',
        priority: 1000,
        addKeyboardShortcuts() {
          return {
            Enter: () => {
              const mention = mentionSuggestionPluginKey.getState(this.editor.state) as
                | { active?: boolean }
                | undefined;
              const emoji = emojiSuggestionPluginKey.getState(this.editor.state) as
                | { active?: boolean }
                | undefined;
              if (mention?.active || emoji?.active) return false;
              sendRef.current();
              return true;
            },
            Escape: () => {
              const mention = mentionSuggestionPluginKey.getState(this.editor.state) as
                | { active?: boolean }
                | undefined;
              const emoji = emojiSuggestionPluginKey.getState(this.editor.state) as
                | { active?: boolean }
                | undefined;
              if (mention?.active || emoji?.active) return false;
              if (editingRef.current && cancelRef.current) {
                cancelRef.current();
                return true;
              }
              return false;
            },
          };
        },
      }),
    [],
  );

  const editor = useEditor({
    extensions: [
      ...createEditorExtensions(),
      Placeholder.configure({
        placeholder: placeholder ?? (isDm ? `Message ${channelName}` : `Message #${channelName}`),
      }),
      submitExtension,
      ...(isDm ? [] : [mentionExtension]),
    ],
    content: editing
      ? flattenMentions(initialContent ?? '')
      : draftKey
        ? useDraftStore.getState().getDraft(draftKey)
        : '',
    onUpdate: ({ editor: ed }) => {
      onTyping?.();
      if (!draftKey) return;
      if (draftTimerRef.current) clearTimeout(draftTimerRef.current);
      draftTimerRef.current = setTimeout(() => {
        useDraftStore.getState().setDraft(draftKey, ed.storage.markdown.getMarkdown());
      }, DRAFT_SAVE_DEBOUNCE_MS);
    },
  });

  const handleSend = useCallback(async () => {
    if (!editor) return;
    const markdown = editor.storage.markdown.getMarkdown().trim();
    if (!markdown) return;
    await onSend(markdown);
    editor.commands.clearContent();
    editor.commands.focus();
    if (draftKey) useDraftStore.getState().clearDraft(draftKey);
  }, [editor, onSend, draftKey]);

  useEffect(() => {
    sendRef.current = () => {
      void handleSend();
    };
  }, [handleSend]);

  useEffect(() => {
    if (editing && editor) editor.commands.focus('end');
  }, [editing, editor]);

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file && onFileUpload) onFileUpload(file);
    if (fileInputRef.current) fileInputRef.current.value = '';
  };

  const isEmpty = !editor || editor.isEmpty;

  return (
    <div className={editing ? 'mt-1' : 'px-4 pb-4'}>
      {onFileUpload && (
        <input ref={fileInputRef} type="file" className="hidden" onChange={handleFileChange} />
      )}
      <div className="bg-slate-800 border border-slate-700 rounded-xl">
        <div className="px-4 pt-3 pb-1">
          <EditorContent editor={editor} className="tiptap-editor" />
        </div>

        <div className="flex items-center justify-between px-3 pb-2 pt-1">
          <div className="flex items-center gap-0.5">
            {onFileUpload && (
              <>
                <button
                  type="button"
                  onClick={() => fileInputRef.current?.click()}
                  disabled={uploading}
                  aria-label="Upload file"
                  className="p-1 text-slate-400 hover:text-slate-200 disabled:text-slate-600 transition cursor-pointer rounded hover:bg-slate-700/60"
                >
                  {uploading ? (
                    <div className="w-3.5 h-3.5 border-2 border-slate-500/30 border-t-slate-400 rounded-full animate-spin" />
                  ) : (
                    <Paperclip className="w-3.5 h-3.5" />
                  )}
                </button>
                <div className="w-px h-4 bg-slate-600/60 mx-0.5" />
              </>
            )}
            <FormattingToolbar editor={editor} />
            <div className="relative">
              <button
                ref={emojiBtnRef}
                type="button"
                onClick={() => setShowEmoji((v) => !v)}
                aria-label="Insert emoji"
                className="p-1 text-slate-400 hover:text-slate-200 transition cursor-pointer rounded hover:bg-slate-700/60"
              >
                <SmilePlus className="w-3.5 h-3.5" />
              </button>
              {showEmoji && (
                <EmojiPicker
                  anchorRef={emojiBtnRef}
                  onSelect={(emoji) => {
                    editor?.chain().focus().insertContent(emoji).run();
                    setShowEmoji(false);
                  }}
                  onClose={() => setShowEmoji(false)}
                />
              )}
            </div>
          </div>

          {editing ? (
            <div className="flex items-center gap-2 text-xs">
              <button
                type="button"
                onClick={handleSend}
                disabled={isEmpty}
                data-qa="message-edit-save"
                className="rounded bg-purple-600 px-2.5 py-1 font-medium text-white transition hover:bg-purple-500 disabled:cursor-not-allowed disabled:opacity-50"
              >
                Save
              </button>
              <button
                type="button"
                onClick={onCancel}
                className="rounded px-2.5 py-1 text-slate-400 transition hover:text-white"
              >
                Cancel
              </button>
            </div>
          ) : (
            <button
              type="button"
              onClick={handleSend}
              disabled={isEmpty}
              className="p-1 text-purple-400 hover:text-purple-300 disabled:text-slate-600 transition cursor-pointer disabled:cursor-not-allowed rounded hover:bg-slate-700/60"
              title="Send (Enter)"
            >
              <Send className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
