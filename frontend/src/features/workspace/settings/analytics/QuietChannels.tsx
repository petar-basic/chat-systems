import { useState } from 'react';
import { Archive } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { Select } from '@/shared/components/Form/Select';
import { ConfirmDialog } from '@/shared/components/ConfirmDialog/ConfirmDialog';
import { QueryState } from '@/shared/components/QueryState/QueryState';
import { toast } from '@/shared/components/Toast';
import { toUserMessage } from '@/lib/errors';
import {
  QUIET_THRESHOLDS,
  useArchiveQuietChannels,
  useQuietChannels,
  type QuietChannel,
  type QuietThreshold,
} from '@/features/workspace/hooks/useAnalytics';
import { dayLabel, dayOf } from './days';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
}

function plural(n: number) {
  return n === 1 ? '1 channel' : `${n.toLocaleString()} channels`;
}

export function QuietChannels({ workspaceId, instanceUrl }: Props) {
  const [days, setDays] = useState<QuietThreshold>(90);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [asking, setAsking] = useState<QuietChannel[] | null>(null);
  const quiet = useQuietChannels(workspaceId, days, instanceUrl);
  const archive = useArchiveQuietChannels(workspaceId, instanceUrl);
  const channels = quiet.data ?? [];
  const selected = channels.filter((c) => picked.has(c.channel_id));

  const toggle = (id: string, on: boolean) =>
    setPicked((current) => {
      const next = new Set(current);
      if (on) next.add(id);
      else next.delete(id);
      return next;
    });

  const confirm = async () => {
    if (!asking) return;
    try {
      const { archived } = await archive.mutateAsync(asking.map((c) => c.channel_id));
      toast.success(`Archived ${plural(archived.length)}`);
      setPicked(new Set());
    } catch (err) {
      throw new Error(toUserMessage(err, 'Could not archive those channels'), { cause: err });
    }
  };

  return (
    <div className="space-y-4" data-qa="analytics-quiet">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <Select
          label="No message for"
          value={days}
          onChange={(e) => {
            setDays(Number(e.target.value) as QuietThreshold);
            setPicked(new Set());
          }}
          dataQa="analytics-quiet-days"
          className="w-48"
        >
          {QUIET_THRESHOLDS.map((n) => (
            <option key={n} value={n}>
              {n} days
            </option>
          ))}
        </Select>
        <Button
          variant="danger"
          size="sm"
          disabled={selected.length === 0}
          onClick={() => setAsking(selected)}
          icon={<Archive className="h-3.5 w-3.5" />}
          data-qa="analytics-quiet-archive-selected"
        >
          {selected.length > 0 ? `Archive ${plural(selected.length)}` : 'Archive selected'}
        </Button>
      </div>

      <QueryState
        isLoading={quiet.isLoading}
        isError={quiet.isError}
        onRetry={() => void quiet.refetch()}
        isEmpty={channels.length === 0}
        empty={
          <p className="rounded-lg border border-dashed border-line py-8 text-center text-sm text-muted">
            Every channel has had a message in the last {days} days.
          </p>
        }
      >
        <ul className="divide-y divide-line rounded-xl border border-line bg-surface">
          {channels.map((c) => (
            <li
              key={c.channel_id}
              className="flex items-center justify-between gap-3 px-4 py-2"
              data-qa="analytics-quiet-row"
            >
              <Checkbox
                label={c.name ?? 'channel'}
                description={
                  <>
                    {c.channel_type === 'private' ? 'Private' : 'Public'}
                    {' · '}
                    {c.last_message_at
                      ? `Last message ${dayLabel(dayOf(c.last_message_at))}`
                      : 'No messages yet'}
                    {' · '}
                    {c.members === 1 ? '1 member' : `${c.members.toLocaleString()} members`}
                  </>
                }
                checked={picked.has(c.channel_id)}
                onChange={(on) => toggle(c.channel_id, on)}
                dataQa="analytics-quiet-pick"
              />
              <Button
                size="sm"
                onClick={() => setAsking([c])}
                icon={<Archive className="h-3.5 w-3.5" />}
                data-qa="analytics-quiet-archive"
                aria-label={`Archive #${c.name ?? 'channel'}`}
              >
                Archive
              </Button>
            </li>
          ))}
        </ul>
      </QueryState>

      {asking && (
        <ConfirmDialog
          title={
            asking.length === 1
              ? `Archive #${asking[0].name ?? 'channel'}?`
              : `Archive ${plural(asking.length)}?`
          }
          description="Nobody can post in an archived channel. It stays readable and searchable, and a channel admin can unarchive it."
          confirmLabel={asking.length === 1 ? 'Archive' : `Archive ${plural(asking.length)}`}
          tone="danger"
          confirmDisabled={archive.isPending}
          onConfirm={confirm}
          onClose={() => setAsking(null)}
          dataQa="analytics-quiet-confirm"
        >
          {asking.length > 1 && (
            <ul className="list-inside list-disc text-sm text-fg-soft">
              {asking.map((c) => (
                <li key={c.channel_id}>#{c.name ?? 'channel'}</li>
              ))}
            </ul>
          )}
        </ConfirmDialog>
      )}
    </div>
  );
}
