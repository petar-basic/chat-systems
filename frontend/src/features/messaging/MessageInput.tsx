import { useRef, useCallback, useMemo, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/shared/components/Button/Button';
import { toUserMessage } from '@/lib/errors';
import { formatDateTime } from '@/lib/datetime';
import { Extension } from '@tiptap/core';
import { useEditor, EditorContent } from '@tiptap/react';
import { editLastOwnMessage } from './messageKeys';
import { enterOutcome } from './enterKey';
import { isApplePlatform, sendKeys, shortcutLabel } from '@/features/navigation';
import { useDisplayStore } from '@/stores/display';
import Placeholder from '@tiptap/extension-placeholder';
import Mention from '@tiptap/extension-mention';
import { createEditorExtensions } from '@/lib/tiptapExtensions';
import { createMentionSuggestion, mentionSuggestionPluginKey } from './mentionSuggestion';
import { emojiSuggestionPluginKey } from './emojiSuggestion';
import { CommandSuggestion } from './commandSuggestion';
import { useSlashCommands, type SlashCommand } from './hooks/useSlashCommands';
import type { MentionItem } from './MentionDropdown';
import EmojiPicker from './EmojiPicker';
import FormattingToolbar from '@/features/messaging/FormattingToolbar';
import type { WorkspaceMember, Channel } from '@/stores/workspace';
import { BarChart3, Clock, Paperclip, Send, SmilePlus, Type } from 'lucide-react';
import { MENTION_SUGGESTION_LIMIT, DRAFT_SAVE_DEBOUNCE_MS, QUERY_KEYS } from '@/shared/constants';
import { useQueryClient } from '@tanstack/react-query';
import { transferFiles } from '@/lib/fileUploads';
import { AttachmentTray } from './AttachmentTray';
import type { ComposerAttachments } from './useComposerAttachments';
import { RESTORE_DRAFT_EVENT, useDraftStore } from '@/stores/drafts';
import { flattenMentions } from '@/lib/mentions';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { useScheduleMessage, type ScheduleTarget } from '@/features/messaging/hooks/useScheduledMessages';
import { toast } from '@/shared/components/Toast';
import { SCHEDULE_PRESETS, formatScheduleHint, toLocalInputValue } from './schedulePresets';
import { useOnClickOutside } from '@/shared/hooks/useOnClickOutside';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';
import { asksBeforeBroadcast, buildMentionItems, matchesMention } from './mentionItems';
import { useUserGroups } from '@/features/workspace';
import { useWorkspaceStore } from '@/stores/workspace';
import { TextInput } from '@/shared/components/Form/TextInput';
import { FilePicker } from '@/shared/components/Form/FilePicker';
import {
  MAX_OPTIONS,
  MIN_OPTIONS,
  PollDialog,
  type PollTarget,
  parsePollCommand,
  useCreatePoll,
} from '@/features/polls';
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
  canBroadcast?: boolean;
  channelMemberCount?: number;
  placeholder?: string;
  draftKey?: string;
  initialContent?: string;
  editing?: boolean;
  autoFocus?: boolean;
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
  slashCommands?: boolean;
}

export default function MessageInput({
  channelName = '',
  members = [],
  channels = [],
  isDm = false,
  canBroadcast = true,
  channelMemberCount = 0,
  placeholder,
  draftKey,
  initialContent,
  editing = false,
  autoFocus = false,
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
  slashCommands = false,
}: Props) {
  const { t } = useTranslation(['messages', 'common', 'polls']);
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
  const enterKey = useDisplayStore((s) => s.enterKey);
  const sendLabel = shortcutLabel(sendKeys(enterKey), isApplePlatform());
  const editingRef = useRef(editing);
  const cancelRef = useRef(onCancel);
  useEffect(() => {
    editingRef.current = editing;
    cancelRef.current = onCancel;
  });

  const workspace = useWorkspaceStore((s) => s.currentWorkspace);
  const { data: groups } = useUserGroups(workspace?.id, workspace?.instanceUrl);
  const mentionItems = useMemo<MentionItem[]>(
    () => buildMentionItems(members, channels, isDm, t, groups ?? [], canBroadcast),
    [members, channels, isDm, t, groups, canBroadcast],
  );
  // The editor is built once, but the people it can mention change with the
  // channel, so the suggestion callback reads the current list through a ref.
  // The compiler cannot see that tiptap only calls this while somebody types.
  const mentionItemsRef = useRef<MentionItem[]>(mentionItems);
  useEffect(() => {
    mentionItemsRef.current = mentionItems;
  }, [mentionItems]);

  useSlashCommands(workspace?.id, workspace?.instanceUrl, slashCommands);
  const queryClient = useQueryClient();
  const [commandExtension] = useState(() =>
    CommandSuggestion.configure({
      commands: () =>
        queryClient.getQueryData<SlashCommand[]>(QUERY_KEYS.slashCommands(workspace?.id ?? '')) ?? [],
    }),
  );

  const mentionExtension = useMemo(
    () =>
      MentionNode.configure({
        HTMLAttributes: { class: 'mention' },
        suggestion: createMentionSuggestion((query) => {
          return mentionItemsRef.current
            .filter((item) => matchesMention(item, query))
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
          const press = (mod: boolean) => {
            const mention = mentionSuggestionPluginKey.getState(this.editor.state) as
              { active?: boolean } | undefined;
            const emoji = emojiSuggestionPluginKey.getState(this.editor.state) as
              { active?: boolean } | undefined;
            const command = this.editor.storage.commandSuggestion;
            const outcome = enterOutcome(useDisplayStore.getState().enterKey, {
              mod,
              picking: !!mention?.active || !!emoji?.active || (!!command?.open && !command.exact),
              inBlock: this.editor.isActive('codeBlock') || this.editor.isActive('listItem'),
            });
            if (outcome === 'default') return false;
            if (outcome === 'newline') return this.editor.commands.setHardBreak();
            sendRef.current();
            return true;
          };
          return {
            Enter: () => press(false),
            'Mod-Enter': () => press(true),
            'Ctrl-Enter': () => press(true),
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
        placeholder:
          placeholder ??
          (isDm
            ? t('composer.placeholderDm', { name: channelName })
            : t('composer.placeholder', { name: channelName })),
      }),
      submitExtension,
      ...(isDm ? [] : [mentionExtension]),
      ...(slashCommands ? [commandExtension] : []),
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
        toast.error(t('composer.waitForUploads'));
        return;
      }
      if (attachments?.failed) {
        toast.error(t('composer.uploadFailed'));
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
        toast.success(t('schedule.scheduled', { time: formatDateTime(sendAt) }));
      } catch (err) {
        toast.error(toUserMessage(err, t('schedule.failed')));
      }
    },
    [editor, scheduleTarget, scheduleMessage, draftKey, attachments, t],
  );

  const [showPoll, setShowPoll] = useState(false);
  const createPoll = useCreatePoll(pollTarget?.channelId ?? '', pollTarget?.instanceUrl);

  const [confirmingBroadcast, setConfirmingBroadcast] = useState(false);

  const handleSend = useCallback(
    async (confirmed = false) => {
      if (!editor) return;
      const markdown = editor.storage.markdown.getMarkdown().trim();
      const pollCommand = pollTarget && !editing ? parsePollCommand(markdown) : null;
      if (pollCommand?.kind === 'invalid') {
        toast.error(
          t(`problems.${pollCommand.problem}`, { ns: 'polls', min: MIN_OPTIONS, max: MAX_OPTIONS }),
        );
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
          toast.error(toUserMessage(err, t('composer.pollFailed')));
        }
        return;
      }
      const files = attachmentsRef.current;
      if (!markdown && !files?.hasReady && !allowEmptyRef.current) return;
      if (files?.busy) {
        toast.error(t('composer.waitForUploads'));
        return;
      }
      if (files?.failed) {
        toast.error(t('composer.uploadFailed'));
        return;
      }
      if (!confirmed && !editing && asksBeforeBroadcast(markdown, channelMemberCount)) {
        setConfirmingBroadcast(true);
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
    },
    [
      editor,
      onSend,
      draftKey,
      pollTarget,
      editing,
      createPoll,
      channelMemberCount,
      t,
      setShowPoll,
      setConfirmingBroadcast,
    ],
  );

  useEffect(() => {
    sendRef.current = () => {
      void handleSend();
    };
  }, [handleSend]);

  useEffect(() => {
    if (editing && editor) editor.commands.focus('end');
  }, [editing, editor]);

  useEffect(() => {
    if (autoFocus && editor && !window.matchMedia?.('(pointer: coarse)')?.matches)
      editor.commands.focus('end');
  }, [autoFocus, editor]);

  useEffect(() => {
    if (!editor || !draftKey) return undefined;
    const restore = (event: Event) => {
      const { key, text } = (event as CustomEvent<{ key: string; text: string }>).detail;
      if (key !== draftKey) return;
      editor.commands.setContent(text);
      editor.commands.focus('end');
    };
    window.addEventListener(RESTORE_DRAFT_EVENT, restore);
    return () => window.removeEventListener(RESTORE_DRAFT_EVENT, restore);
  }, [editor, draftKey]);

  const isEmpty = !editor || (editor.isEmpty && !attachments?.hasReady && !allowEmpty);
  const sendBlocked = isEmpty || !!attachments?.busy || !!attachments?.failed;

  return (
    // The bottom padding clears the iOS home indicator; without it the send
    // button sits under it and cannot be tapped.
    <div className={editing ? 'mt-1' : 'px-4 pb-4 max-sm:pb-[max(1rem,env(safe-area-inset-bottom))]'}>
      <div
        className="@container bg-surface border border-line rounded-xl"
        data-composer={editing ? undefined : true}
      >
        {attachments && (
          <AttachmentTray
            items={attachments.items}
            onRemove={attachments.remove}
            onRetry={attachments.retry}
          />
        )}
        <div className="px-4 pt-3 pb-1">
          <EditorContent editor={editor} className="tiptap-editor" />
        </div>

        <div className="flex items-start justify-between gap-1 px-3 pb-2 pt-1">
          <div className="flex flex-wrap items-center gap-0.5 min-w-0">
            {attachments && (
              <FilePicker
                label={t('composer.upload')}
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
                aria-label={t('composer.poll')}
                data-tooltip={t('composer.poll')}
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
              aria-label={showFormatting ? t('composer.hideFormatting') : t('composer.showFormatting')}
              data-tooltip={showFormatting ? t('composer.hideFormatting') : t('composer.showFormatting')}
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
                aria-label={t('composer.emoji')}
                data-tooltip={t('composer.emoji')}
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

                onClick={() => void handleSend()}
                disabled={isEmpty}
                data-qa="message-edit-save"
              >
                {t('composer.save')}
              </Button>
              <button
                type="button"
                onClick={onCancel}
                className="rounded px-2.5 py-1 text-muted transition hover:text-fg"
              >
                {t('cancel', { ns: 'common' })}
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
                    aria-label={t('schedule.label')}
                    aria-expanded={showSchedule}
                    data-qa="schedule-open"
                    className="p-1 text-muted hover:text-fg disabled:text-faint transition cursor-pointer disabled:cursor-not-allowed rounded hover:bg-raised/60"
                    data-tooltip={t('schedule.hint')}
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
                          key={preset.name}
                          type="button"
                          onClick={() => void handleSchedule(preset.at())}
                          data-qa="schedule-preset"
                          className="w-full px-3 py-2 flex items-center justify-between gap-2 text-left text-sm text-fg-dim hover:bg-raised/60 transition cursor-pointer"
                        >
                          <span>{t(`schedule.${preset.name}`)}</span>
                          <span className="text-2xs text-subtle">{formatScheduleHint(preset.at())}</span>
                        </button>
                      ))}

                      <div className="mt-1 border-t border-line/70 px-3 py-2">
                        <TextInput
                          label={t('schedule.pickTime')}
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
                          <p className="mt-1 text-2xs text-danger" data-qa="schedule-error">
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
                              setScheduleError(t('schedule.needTime'));
                              return;
                            }
                            if (at.getTime() <= Date.now()) {
                              setScheduleError(t('schedule.past'));
                              return;
                            }
                            void handleSchedule(at);
                          }}
                          data-qa="schedule-custom-submit"
                        >
                          {t('schedule.submit')}
                        </Button>
                      </div>
                    </div>
                  )}
                </div>
              )}
              <button
                type="button"
                onClick={() => void handleSend()}
                disabled={sendBlocked}
                data-qa="composer-send"
                className="p-1 text-accent hover:text-accent-soft disabled:text-faint transition cursor-pointer disabled:cursor-not-allowed rounded hover:bg-raised/60"
                data-tooltip={t('composer.sendHint', { keys: sendLabel })}
                aria-label={t('composer.send')}
              >
                <Send className="w-3.5 h-3.5" />
              </button>
            </div>
          )}
        </div>
      </div>
      {showPoll && pollTarget && <PollDialog target={pollTarget} onClose={() => setShowPoll(false)} />}
      {confirmingBroadcast && (
        <ConfirmDialog
          title={t('composer.broadcastTitle', { count: channelMemberCount - 1 })}
          confirmLabel={t('composer.send')}
          tone="primary"
          onConfirm={() => handleSend(true)}
          onClose={() => setConfirmingBroadcast(false)}
          dataQa="broadcast-confirm"
        />
      )}
    </div>
  );
}
