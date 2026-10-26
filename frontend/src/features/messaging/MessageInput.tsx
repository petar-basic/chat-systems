import { useRef, useCallback, useMemo, useEffect, useState } from 'react';
import { Button } from '@/shared/components/Button/Button';
import { toUserMessage } from '@/lib/errors';
import { formatDateTime } from '@/lib/datetime';
import { Extension } from '@tiptap/core';
import { useEditor, EditorContent } from '@tiptap/react';
import { editLastOwnMessage } from './messageKeys';
import Placeholder from '@tiptap/extension-placeholder';
import Mention from '@tiptap/extension-mention';
import { createEditorExtensions } from '@/lib/tiptapExtensions';
import { createMentionSuggestion, mentionSuggestionPluginKey } from './mentionSuggestion';
import { emojiSuggestionPluginKey } from './emojiSuggestion';
import type { MentionItem } from './MentionDropdown';
import EmojiPicker from './EmojiPicker';
import FormattingToolbar from '@/features/messaging/FormattingToolbar';
import type { WorkspaceMember, Channel } from '@/stores/workspace';
import { BarChart3, Clock, Paperclip, Send, SmilePlus, Type } from 'lucide-react';
import { MENTION_SUGGESTION_LIMIT, DRAFT_SAVE_DEBOUNCE_MS } from '@/shared/constants';
import { transferFiles } from '@/lib/fileUploads';
import { AttachmentTray } from './AttachmentTray';
import type { ComposerAttachments } from './useComposerAttachments';
import { useDraftStore } from '@/stores/drafts';
import { flattenMentions } from '@/lib/mentions';
import { useScheduleMessage, type ScheduleTarget } from '@/features/messaging/hooks/useScheduledMessages';
import { toast } from '@/shared/components/Toast';
import { SCHEDULE_PRESETS, formatScheduleHint, toLocalInputValue } from './schedulePresets';
import { useOnClickOutside } from '@/shared/hooks/useOnClickOutside';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';
import { buildMentionItems } from './mentionItems';
import { useUserGroups } from '@/features/workspace';
import { useWorkspaceStore } from '@/stores/workspace';
import { TextInput } from '@/shared/components/Form/TextInput';
import { FilePicker } from '@/shared/components/Form/FilePicker';
import { PollDialog, type PollTarget, parsePollCommand, useCreatePoll } from '@/features/polls';
import { ShortcutMenu } from '@/features/workflows';

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
  allowEmpty?: boolean;
  onSend: (content: string) => Promise<void>;
  onCancel?: () => void;
  attachments?: ComposerAttachments;
  onTyping?: () => void;
  workspaceId?: string;
  instanceUrl?: string;
  scheduleTarget?: ScheduleTarget;
  pollTarget?: PollTarget;
  shortcutChannelId?: string;
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
  allowEmpty = false,
  onSend,
  onCancel,
  attachments,
  onTyping,
  workspaceId,
  instanceUrl,
  scheduleTarget,
  pollTarget,
  shortcutChannelId,
}: Props) {
  const attachmentsRef = useRef(attachments);
  const allowEmptyRef = useRef(allowEmpty);
  useEffect(() => {
    attachmentsRef.current = attachments;
    allowEmptyRef.current = allowEmpty;
  });

  const addFiles = useCallback((files: File[]) => {
    attachmentsRef.current?.add(files);
  }, []);
  const draftTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const sendRef = useRef<() => void>(() => {});
  const emojiBtnRef = useRef<HTMLButtonElement>(null);
  const [showEmoji, setShowEmoji] = useState(false);
  const [showFormatting, setShowFormatting] = useState(false);
  const editingRef = useRef(editing);
  const cancelRef = useRef(onCancel);
  useEffect(() => {
    editingRef.current = editing;
    cancelRef.current = onCancel;
  });

  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const { data: groups } = useUserGroups(workspace?.id, workspace?.instanceUrl);
  const mentionItems = useMemo<MentionItem[]>(
    () => buildMentionItems(members, channels, isDm, groups ?? []),
    [members, channels, isDm, groups],
  );
  // The editor is built once, but the people it can mention change with the
  // channel, so the suggestion callback reads the current list through a ref.
  // The compiler cannot see that tiptap only calls this while somebody types.
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
                { active?: boolean } | undefined;
              const emoji = emojiSuggestionPluginKey.getState(this.editor.state) as
                { active?: boolean } | undefined;
              if (mention?.active || emoji?.active) return false;
              sendRef.current();
              return true;
            },
            ArrowUp: () => {
              if (editingRef.current || !this.editor.isEmpty) return false;
              return editLastOwnMessage(this.editor.view.dom);
            },
            Escape: () => {
              const mention = mentionSuggestionPluginKey.getState(this.editor.state) as
                { active?: boolean } | undefined;
              const emoji = emojiSuggestionPluginKey.getState(this.editor.state) as
                { active?: boolean } | undefined;
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
    editorProps: {
      handlePaste: (_view, event) => {
        const files = transferFiles(event.clipboardData);
        if (files.length === 0 || !attachmentsRef.current) return false;
        event.preventDefault();
        addFiles(files);
        return true;
      },
      handleDrop: (_view, event) => {
        const files = transferFiles((event as DragEvent).dataTransfer);
        if (files.length === 0 || !attachmentsRef.current) return false;
        event.preventDefault();
        addFiles(files);
        return true;
      },
    },
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
  // Stamped when the menu opens: reading the clock during render makes the
  // component impure, and a minute from *now* is only meaningful at that moment.
  const [earliestSendAt, setEarliestSendAt] = useState('');
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
      const files = attachments?.items.filter((item) => item.status === 'ready') ?? [];
      if (!markdown && files.length === 0) return;
      if (attachments?.busy) {
        toast.error('Wait for the files to finish uploading');
        return;
      }
      try {
        await scheduleMessage.mutateAsync({
          target: scheduleTarget,
          content: markdown,
          sendAt,
          attachmentIds: files.map((item) => item.fileId ?? ''),
        });
        attachments?.take();
        editor.commands.clearContent();
        if (draftKey) useDraftStore.getState().clearDraft(draftKey);
        setShowSchedule(false);
        setCustomSendAt('');
        setScheduleError(null);
        toast.success(`Scheduled for ${formatDateTime(sendAt)}`);
      } catch (err) {
        toast.error(toUserMessage(err, 'Could not schedule that message'));
      }
    },
    [editor, scheduleTarget, scheduleMessage, draftKey, attachments],
  );

  const [showPoll, setShowPoll] = useState(false);
  const createPoll = useCreatePoll(pollTarget?.channelId ?? '', pollTarget?.instanceUrl);

  const handleSend = useCallback(async () => {
    if (!editor) return;
    const markdown = editor.storage.markdown.getMarkdown().trim();
    const pollCommand = pollTarget && !editing ? parsePollCommand(markdown) : null;
    if (pollCommand?.kind === 'invalid') {
      toast.error(pollCommand.message);
      return;
    }
    if (pollCommand) {
      editor.commands.clearContent();
      if (draftKey) useDraftStore.getState().clearDraft(draftKey);
      if (pollCommand.kind === 'dialog') {
        setShowPoll(true);
        return;
      }
      try {
        await createPoll.mutateAsync({
          question: pollCommand.question,
          options: pollCommand.options,
          thread_parent_id: pollTarget?.threadParentId ?? null,
        });
      } catch (err) {
        editor.commands.setContent(markdown);
        toast.error(toUserMessage(err, "Couldn't create the poll"));
      }
      return;
    }
    const files = attachmentsRef.current;
    if (!markdown && !files?.hasReady && !allowEmptyRef.current) return;
    if (files?.busy) {
      toast.error('Wait for the files to finish uploading');
      return;
    }
    editor.commands.clearContent();
    editor.commands.focus();
    if (draftKey) useDraftStore.getState().clearDraft(draftKey);
    try {
      await onSend(markdown);
    } catch {
      editor.commands.setContent(markdown);
      if (draftKey) useDraftStore.getState().setDraft(draftKey, markdown);
    }
  }, [editor, onSend, draftKey, pollTarget, editing, createPoll]);

  useEffect(() => {
    sendRef.current = () => {
      void handleSend();
    };
  }, [handleSend]);

  useEffect(() => {
    if (editing && editor) editor.commands.focus('end');
  }, [editing, editor]);

  const isEmpty = !editor || (editor.isEmpty && !attachments?.hasReady && !allowEmpty);
  const sendBlocked = isEmpty || !!attachments?.busy;

  return (
    // The bottom padding clears the iOS home indicator; without it the send
    // button sits under it and cannot be tapped.
    <div className={editing ? 'mt-1' : 'px-4 pb-4 max-sm:pb-[max(1rem,env(safe-area-inset-bottom))]'}>
      <div
        className="@container bg-surface border border-line rounded-xl"
        data-composer={editing ? undefined : true}
      >
        {attachments && <AttachmentTray items={attachments.items} onRemove={attachments.remove} />}
        <div className="px-4 pt-3 pb-1">
          <EditorContent editor={editor} className="tiptap-editor" />
        </div>

        <div className="flex items-start justify-between gap-1 px-3 pb-2 pt-1">
          <div className="flex flex-wrap items-center gap-0.5 min-w-0">
            {attachments && (
              <FilePicker
                label="Upload file"
                multiple
                onFiles={addFiles}
                dataQa="composer-upload"
                className="p-1 text-muted hover:text-fg-soft transition rounded hover:bg-raised/60"
              >
                <Paperclip className="w-3.5 h-3.5" />
              </FilePicker>
            )}
            {pollTarget && !editing && (
              <button
                type="button"
                onClick={() => setShowPoll(true)}
                aria-label="Create a poll"
                title="Create a poll"
                data-qa="composer-poll"
                className="p-1 text-muted hover:text-fg-soft transition cursor-pointer rounded hover:bg-raised/60"
              >
                <BarChart3 className="w-3.5 h-3.5" />
              </button>
            )}
            {shortcutChannelId && !editing && (
              <ShortcutMenu channelId={shortcutChannelId} instanceUrl={instanceUrl} />
            )}
            {(attachments || (pollTarget && !editing)) && <div className="w-px h-4 bg-elevated/60 mx-0.5" />}
            {/* Twelve formatting buttons in a composer this narrow is more
                chrome than composer, and markdown shortcuts still work while it
                is folded away. */}
            <div className={showFormatting ? 'contents' : '@max-md:hidden contents'}>
              <FormattingToolbar editor={editor} />
            </div>
            <button
              type="button"
              onClick={() => setShowFormatting((open) => !open)}
              aria-label={showFormatting ? 'Hide formatting' : 'Show formatting'}
              aria-pressed={showFormatting}
              data-qa="composer-formatting-toggle"
              className="@md:hidden p-1 text-muted hover:text-fg-soft transition cursor-pointer rounded hover:bg-raised/60"
            >
              <Type className="w-3.5 h-3.5" />
            </button>
            <div className="relative">
              <button
                ref={emojiBtnRef}
                type="button"
                onClick={() => setShowEmoji((v) => !v)}
                aria-label="Insert emoji"
                className="p-1 text-muted hover:text-fg-soft transition cursor-pointer rounded hover:bg-raised/60"
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
            <div className="flex shrink-0 items-center gap-2 text-xs">
              <Button
                variant="primary"
                size="sm"

                onClick={handleSend}
                disabled={isEmpty}
                data-qa="message-edit-save"
              >
                Save
              </Button>
              <button
                type="button"
                onClick={onCancel}
                className="rounded px-2.5 py-1 text-muted transition hover:text-fg"
              >
                Cancel
              </button>
            </div>
          ) : (
            <div className="flex shrink-0 items-center gap-0.5">
              {canSchedule && (
                <div className="relative">
                  <button
                    type="button"
                    onClick={() => {
                      setEarliestSendAt(toLocalInputValue(new Date(Date.now() + 60_000)));
                      setShowSchedule((open) => !open);
                    }}
                    disabled={isEmpty}
                    aria-label="Schedule message"
                    aria-expanded={showSchedule}
                    data-qa="schedule-open"
                    className="p-1 text-muted hover:text-fg disabled:text-faint transition cursor-pointer disabled:cursor-not-allowed rounded hover:bg-raised/60"
                    title="Send later"
                  >
                    <Clock className="w-3.5 h-3.5" />
                  </button>
                  {showSchedule && (
                    <div
                      ref={scheduleMenuRef}
                      data-qa="schedule-menu"
                      className="absolute bottom-full right-0 mb-2 w-64 bg-surface border border-line rounded-lg shadow-xl py-1 z-20"
                    >
                      {SCHEDULE_PRESETS.map((preset) => (
                        <button
                          key={preset.label}
                          type="button"
                          onClick={() => void handleSchedule(preset.at())}
                          data-qa="schedule-preset"
                          className="w-full px-3 py-2 flex items-center justify-between gap-2 text-left text-sm text-fg-dim hover:bg-raised/60 transition cursor-pointer"
                        >
                          <span>{preset.label}</span>
                          <span className="text-[11px] text-subtle">{formatScheduleHint(preset.at())}</span>
                        </button>
                      ))}

                      <div className="mt-1 border-t border-line/70 px-3 py-2">
                        <TextInput
                          label="Pick a time"
                          compact
                          type="datetime-local"
                          value={customSendAt}
                          min={earliestSendAt}
                          onChange={(e) => {
                            setCustomSendAt(e.target.value);
                            setScheduleError(null);
                          }}
                          dataQa="schedule-custom-input"
                        />
                        {scheduleError && (
                          <p className="mt-1 text-[11px] text-danger" data-qa="schedule-error">
                            {scheduleError}
                          </p>
                        )}
                        <Button
                          variant="primary"
                          fullWidth
                          className="mt-2"

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
                        >
                          Schedule
                        </Button>
                      </div>
                    </div>
                  )}
                </div>
              )}
              <button
                type="button"
                onClick={handleSend}
                disabled={sendBlocked}
                data-qa="composer-send"
                className="p-1 text-accent hover:text-accent-soft disabled:text-faint transition cursor-pointer disabled:cursor-not-allowed rounded hover:bg-raised/60"
                title="Send (Enter)"
                aria-label="Send"
              >
                <Send className="w-3.5 h-3.5" />
              </button>
            </div>
          )}
        </div>
      </div>
      {showPoll && pollTarget && <PollDialog target={pollTarget} onClose={() => setShowPoll(false)} />}
    </div>
  );
}
