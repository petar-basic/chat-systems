import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import i18n from '@/lib/i18n';
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

describe('CreateChannelModal in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the form and says what is wrong with the name', async () => {
    await i18n.changeLanguage('ja');
    render(
      <CreateChannelModal
        kinds={['public', 'private']}
        channels={[{ id: 'ch-1', name: 'design' } as never]}
        onCreate={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByRole('heading', { name: 'チャンネルを作成' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: /^公開/ })).toBeChecked();
    expect(screen.getByText('招待された人だけが閲覧・参加できます。')).toBeInTheDocument();
    expect(
      screen.getByText('ワークスペースとこのチャンネルの管理者だけが投稿できます。'),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByTestId('create-channel-submit'));
    expect(await screen.findByText('チャンネル名を入力してください。')).toBeInTheDocument();

    fireEvent.change(screen.getByTestId('create-channel-name'), { target: { value: 'Design' } });
    fireEvent.click(screen.getByTestId('create-channel-submit'));
    expect(await screen.findByText('同じ名前のチャンネルがすでにあります。')).toBeInTheDocument();
  });
});
