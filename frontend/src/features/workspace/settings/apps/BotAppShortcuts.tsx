import { useState, type FormEvent } from 'react';
import { Plus, Trash2 } from 'lucide-react';
import { useBotAppShortcuts, useSaveBotAppShortcuts, type ShortcutDraft } from '@/features/apps';
import { Button } from '@/shared/components/Button/Button';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { Select } from '@/shared/components/Form/Select';
import { TextInput } from '@/shared/components/Form/TextInput';
import { toUserMessage } from '@/lib/errors';

const PER_KIND = 5;

interface Props {
  appId: string;
}

export function BotAppShortcuts({ appId }: Props) {
  const listed = useBotAppShortcuts(appId);
  if (!listed.data) return null;
  return (
    <ShortcutsForm
      key={listed.dataUpdatedAt}
      appId={appId}
      initial={listed.data.map(({ kind, name, description, callback_id }) => ({
        kind,
        name,
        description,
        callback_id,
      }))}
    />
  );
}

interface FormProps {
  appId: string;
  initial: ShortcutDraft[];
}

function ShortcutsForm({ appId, initial }: FormProps) {
  const save = useSaveBotAppShortcuts(appId);
  const [drafts, setDrafts] = useState<ShortcutDraft[]>(initial);
  const count = (kind: ShortcutDraft['kind']) => drafts.filter((d) => d.kind === kind).length;
  const full = count('global') >= PER_KIND && count('message') >= PER_KIND;

  const change = (index: number, patch: Partial<ShortcutDraft>) =>
    setDrafts((current) => current.map((draft, i) => (i === index ? { ...draft, ...patch } : draft)));

  const add = () =>
    setDrafts((current) => [
      ...current,
      {
        kind: count('global') < PER_KIND ? 'global' : 'message',
        name: '',
        description: '',
        callback_id: '',
      },
    ]);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    save.mutate(drafts);
  };

  return (
    <form onSubmit={submit} className="space-y-3 border-t border-line/50 pt-3" data-qa="bot-app-shortcuts">
      <div>
        <h4 className="text-xs font-medium text-fg-dim">Shortcuts</h4>
        <p className="text-xs text-muted">
          Global shortcuts sit in the composer&apos;s shortcut menu and in the quick switcher; message
          shortcuts sit under More actions on messages in conversations the app is in. Using one sends the app
          its callback ID. At most {PER_KIND} of each.
        </p>
      </div>
      {drafts.map((draft, index) => (
        <div
          key={index}
          className="grid grid-cols-1 gap-2 rounded-lg bg-raised/40 p-3 sm:grid-cols-2"
          data-qa="bot-app-shortcut"
        >
          <Select
            label="Where"
            value={draft.kind}
            onChange={(e) => change(index, { kind: e.target.value as ShortcutDraft['kind'] })}
            dataQa="bot-app-shortcut-kind"
          >
            <option value="global">Global</option>
            <option value="message">On a message</option>
          </Select>
          <TextInput
            label="Callback ID"
            value={draft.callback_id}
            onChange={(e) => change(index, { callback_id: e.target.value })}
            placeholder="new_request"
            dataQa="bot-app-shortcut-callback"
          />
          <TextInput
            label="Name"
            value={draft.name}
            onChange={(e) => change(index, { name: e.target.value })}
            placeholder="New request"
            dataQa="bot-app-shortcut-name"
          />
          <TextInput
            label="Description"
            value={draft.description}
            onChange={(e) => change(index, { description: e.target.value })}
            placeholder="Ask the help desk for something"
            dataQa="bot-app-shortcut-description"
          />
          <div className="sm:col-span-2 flex justify-end">
            <IconButton
              label="Remove this shortcut"
              icon={<Trash2 className="h-4 w-4" />}
              tone="danger"
              onClick={() => setDrafts((current) => current.filter((_, i) => i !== index))}
              data-qa="bot-app-shortcut-remove"
            />
          </div>
        </div>
      ))}
      <div className="flex flex-wrap items-center gap-2">
        <Button type="button" size="sm" onClick={add} disabled={full} data-qa="bot-app-shortcut-add">
          <Plus className="h-3.5 w-3.5" /> Add a shortcut
        </Button>
        <Button
          type="submit"
          variant="primary"
          size="sm"
          loading={save.isPending}
          data-qa="bot-app-shortcuts-save"
        >
          Save shortcuts
        </Button>
        {save.isSuccess && !save.isPending && (
          <span className="text-xs text-muted" data-qa="bot-app-shortcuts-saved">
            Saved
          </span>
        )}
      </div>
      {save.error && (
        <p role="alert" className="text-sm text-danger">
          {toUserMessage(save.error, 'The shortcuts could not be saved')}
        </p>
      )}
    </form>
  );
}
