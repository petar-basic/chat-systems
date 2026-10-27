import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { CreateChannelModal } from './CreateChannelModal';
import type { ChannelKind } from '@/lib/channelPermissions';

function renderModal(kinds: ChannelKind[]) {
  const onCreate = vi.fn().mockResolvedValue(undefined);
  render(<CreateChannelModal kinds={kinds} channels={[]} onCreate={onCreate} onClose={vi.fn()} />);
  return onCreate;
}

describe('CreateChannelModal', () => {
  it('offers both kinds to someone who may create either', () => {
    renderModal(['public', 'private']);
    expect(screen.getByTestId('create-channel-public')).toBeInTheDocument();
    expect(screen.getByTestId('create-channel-private')).toBeInTheDocument();
  });

  it('offers only the kind the workspace lets this person create, and creates that kind', async () => {
    const onCreate = renderModal(['private']);
    expect(screen.queryByTestId('create-channel-public')).toBeNull();
    expect(screen.getByRole('radio', { name: 'Private' })).toBeChecked();

    fireEvent.change(screen.getByTestId('create-channel-name'), { target: { value: 'secret-plans' } });
    fireEvent.click(screen.getByTestId('create-channel-submit'));

    await waitFor(() => expect(onCreate).toHaveBeenCalledWith(expect.objectContaining({ isPrivate: true })));
  });

  it('starts on public when only public channels are allowed', () => {
    renderModal(['public']);
    expect(screen.queryByTestId('create-channel-private')).toBeNull();
    expect(screen.getByRole('radio', { name: 'Public' })).toBeChecked();
  });
});
