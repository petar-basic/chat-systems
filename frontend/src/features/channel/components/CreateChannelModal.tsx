import { useState, type FormEvent } from 'react';
import { Modal } from '@/shared/components/Modal/Modal';
import type { CreateChannelDraft } from '@/models/channel';
import type { Channel } from '@/stores/workspace';

interface Props {
  channels: Channel[];
  onCreate: (draft: CreateChannelDraft) => Promise<void>;
  onClose: () => void;
}

export function CreateChannelModal({ channels, onCreate, onClose }: Props) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [isPrivate, setIsPrivate] = useState(false);
  const [announcementOnly, setAnnouncementOnly] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) {
      setError('Enter a channel name.');
      return;
    }
    if (channels.some((ch) => ch.name?.toLowerCase() === trimmed.toLowerCase())) {
      setError('A channel with that name already exists.');
      return;
    }
    setError(null);
    setCreating(true);
    try {
      await onCreate({
        name: trimmed,
        description: description.trim() || undefined,
        isPrivate,
        announcementOnly,
      });
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not create the channel.');
    } finally {
      setCreating(false);
    }
  };

  return (
    <Modal title="Create Channel" onClose={onClose} dataQa="create-channel-modal">
      <form onSubmit={handleSubmit} noValidate>
        <h2 className="text-lg font-bold text-fg mb-4">Create Channel</h2>
        <input
          aria-label="Channel name"
          type="text"
          value={name}
          onChange={(e) => {
            setName(e.target.value);
            if (error) setError(null);
          }}
          placeholder="Channel name"
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? 'create-channel-error' : undefined}
          data-qa="create-channel-name"
          className={`w-full px-4 py-3 bg-raised/50 border rounded-lg text-fg placeholder-muted focus:outline-none focus:ring-2 ${
            error ? 'border-red-500/60 focus:ring-red-500' : 'border-line-strong focus:ring-purple-500'
          }`}
        />
        {error && (
          <p id="create-channel-error" data-qa="create-channel-error" className="mt-2 text-sm text-danger">
            {error}
          </p>
        )}
        <label
          htmlFor="create-channel-description"
          className="mt-4 block text-sm font-medium text-fg-dim mb-1.5"
        >
          Description <span className="text-muted font-normal">(optional)</span>
        </label>
        <textarea
          id="create-channel-description"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          rows={2}
          placeholder="What is this channel for?"
          data-qa="create-channel-description"
          className="w-full px-4 py-2.5 bg-raised/50 border border-line-strong rounded-lg text-fg text-sm placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500 resize-none"
        />

        <label className="mt-4 flex items-start gap-3 cursor-pointer" data-qa="create-channel-private">
          <input
            type="checkbox"
            checked={isPrivate}
            onChange={(e) => setIsPrivate(e.target.checked)}
            className="mt-0.5 w-4 h-4 accent-purple-500"
          />
          <span>
            <span className="block text-sm font-medium text-fg-dim">Private channel</span>
            <span className="block text-xs text-muted">
              Only people who are invited can find it or read it.
            </span>
          </span>
        </label>

        <label className="mt-3 flex items-start gap-3 cursor-pointer" data-qa="create-channel-announcement">
          <input
            type="checkbox"
            checked={announcementOnly}
            onChange={(e) => setAnnouncementOnly(e.target.checked)}
            className="mt-0.5 w-4 h-4 accent-purple-500"
          />
          <span>
            <span className="block text-sm font-medium text-fg-dim">Announcement channel</span>
            <span className="block text-xs text-muted">
              Only admins can post. Everyone else can still read and react.
            </span>
          </span>
        </label>

        <div className="flex justify-end gap-2 mt-5">
          <button type="button" onClick={onClose} className="px-4 py-2 text-muted hover:text-fg transition">
            Cancel
          </button>
          <button
            type="submit"
            disabled={creating}
            className="px-4 py-2 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white rounded-lg transition disabled:cursor-not-allowed"
          >
            {creating ? 'Creating…' : 'Create'}
          </button>
        </div>
      </form>
    </Modal>
  );
}
