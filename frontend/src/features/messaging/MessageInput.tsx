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
import { Clock, Paperclip, Send, SmilePlus } from 'lucide-react';
import { MENTION_SUGGESTION_LIMIT, DRAFT_SAVE_DEBOUNCE_MS } from '@/shared/constants';
import { useDraftStore } from '@/stores/drafts';
import { flattenMentions } from '@/lib/mentions';
import { useScheduleMessage, type ScheduleTarget } from '@/hooks/queries/useScheduledMessages';
import { toast } from '@/shared/components/Toast';
import { SCHEDULE_PRESETS, formatScheduleHint, toLocalInputValue } from './schedulePresets';
import { useOnClickOutside } from '@/shared/hooks/useOnClickOutside';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';
import { buildMentionItems } from './mentionItems';
import { useUserGroups } from '@/hooks/queries/useUserGroups';
import { useWorkspaceStore } from '@/stores/workspace';

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
  workspaceId?: string;
  instanceUrl?: string;
  scheduleTarget?: ScheduleTarget;
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
  workspaceId,
  instanceUrl,
  scheduleTarget,
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

  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const { data: groups } = useUserGroups(workspace?.id, workspace?.instanceUrl);
  const mentionItems = useMemo<MentionItem[]>(
    () => buildMentionItems(members, channels, isDm, groups ?? []),
    [members, channels, isDm, groups],
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
    shouldRerenderOnTransaction: true,
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

  const [showSchedule, setShowSchedule] = useState(false);
  const [customSendAt, setCustomSendAt] = useState('');
  const [scheduleError, setScheduleError] = useState<string | null>(null);
  const scheduleMenuRef = useRef<HTMLDivElement>(null);
  useOnClickOutside(scheduleMenuRef, () => setShowSchedule(false), showSchedule);
  useEscapeToClose(() => setShowSchedule(false), showSchedule);
  const scheduleMessage = useScheduleMessage(workspaceId ?? '', instanceUrl);
  const canSchedule = !!workspaceId && !!scheduleTarget && !editing;

  const handleSchedule = useCallback(
    async (sendAt: Date) => {
      if (!editor || !scheduleTarget) return;
      const markdown = editor.storage.markdown.getMarkdown().trim();
      if (!markdown) return;
      try {
        await scheduleMessage.mutateAsync({ target: scheduleTarget, content: markdown, sendAt });
        editor.commands.clearContent();
        if (draftKey) useDraftStore.getState().clearDraft(draftKey);
        setShowSchedule(false);
        setCustomSendAt('');
        setScheduleError(null);
        toast.success(`Scheduled for ${sendAt.toLocaleString()}`);
      } catch (err) {
        toast.error((err as { message?: string })?.message || 'Could not schedule that message');
      }
    },
    [editor, scheduleTarget, scheduleMessage, draftKey],
  );

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
            <div className="flex items-center gap-0.5">
              {canSchedule && (
                <div className="relative">
                  <button
                    type="button"
                    onClick={() => setShowSchedule((open) => !open)}
                    disabled={isEmpty}
                    aria-label="Schedule message"
                    aria-expanded={showSchedule}
                    data-qa="schedule-open"
                    className="p-1 text-slate-400 hover:text-white disabled:text-slate-600 transition cursor-pointer disabled:cursor-not-allowed rounded hover:bg-slate-700/60"
                    title="Send later"
                  >
                    <Clock className="w-3.5 h-3.5" />
                  </button>
                  {showSchedule && (
                    <div
                      ref={scheduleMenuRef}
                      data-qa="schedule-menu"
                      className="absolute bottom-full right-0 mb-2 w-64 bg-slate-800 border border-slate-700 rounded-lg shadow-xl py-1 z-20"
                    >
                      {SCHEDULE_PRESETS.map((preset) => (
                        <button
                          key={preset.label}
                          type="button"
                          onClick={() => void handleSchedule(preset.at())}
                          data-qa="schedule-preset"
                          className="w-full px-3 py-2 flex items-center justify-between gap-2 text-left text-sm text-slate-300 hover:bg-slate-700/60 transition cursor-pointer"
                        >
                          <span>{preset.label}</span>
                          <span className="text-[11px] text-slate-500">
                            {formatScheduleHint(preset.at())}
                          </span>
                        </button>
                      ))}

                      <div className="mt-1 border-t border-slate-700/70 px-3 py-2">
                        <label
                          htmlFor={`schedule-custom-${draftKey ?? 'composer'}`}
                          className="block text-[11px] uppercase tracking-wider text-slate-400 mb-1"
                        >
                          Pick a time
                        </label>
                        <input
                          id={`schedule-custom-${draftKey ?? 'composer'}`}
                          type="datetime-local"
                          value={customSendAt}
                          min={toLocalInputValue(new Date(Date.now() + 60_000))}
                          onChange={(e) => {
                            setCustomSendAt(e.target.value);
                            setScheduleError(null);
                          }}
                          data-qa="schedule-custom-input"
                          className="w-full px-2 py-1.5 bg-slate-700/50 border border-slate-600 rounded text-sm text-white focus:outline-none focus:ring-2 focus:ring-purple-500 [color-scheme:dark]"
                        />
                        {scheduleError && (
                          <p className="mt-1 text-[11px] text-red-400" data-qa="schedule-error">
                            {scheduleError}
                          </p>
                        )}
                        <button
                          type="button"
                          onClick={() => {
                            const at = new Date(customSendAt);
                            if (!customSendAt || Number.isNaN(at.getTime())) {
                              setScheduleError('Pick a date and time first');
                              return;
                            }
                            if (at.getTime() <= Date.now()) {
                              setScheduleError('That time has already passed');
                              return;
                            }
                            void handleSchedule(at);
                          }}
                          data-qa="schedule-custom-submit"
                          className="mt-2 w-full px-3 py-1.5 bg-purple-600 hover:bg-purple-500 text-white text-sm font-medium rounded transition cursor-pointer"
                        >
                          Schedule
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              )}
              <button
                type="button"
                onClick={handleSend}
                disabled={isEmpty}
                className="p-1 text-purple-400 hover:text-purple-300 disabled:text-slate-600 transition cursor-pointer disabled:cursor-not-allowed rounded hover:bg-slate-700/60"
                title="Send (Enter)"
              >
                <Send className="w-3.5 h-3.5" />
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
