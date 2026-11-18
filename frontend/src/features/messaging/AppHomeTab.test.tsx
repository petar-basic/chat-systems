import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { AppHomeTab } from './AppHomeTab';

const home = vi.hoisted(() => ({
  tab: { isPending: false, isError: false, data: undefined as unknown, refetch: vi.fn() },
  opened: vi.fn(),
  act: vi.fn(),
}));

vi.mock('@/features/apps', () => ({
  useAppHome: () => home.tab,
  useAppHomeOpened: () => ({ mutate: home.opened }),
  useAppHomeAction: () => ({ mutate: home.act, isPending: false, variables: undefined }),
}));

const helpdesk = {
  app_id: 'app-1',
  bot_user_id: 'bot-1',
  name: 'Helpdesk',
  description: 'Ask the help desk for anything',
  icon_url: null,
  interactive: true,
  home: true,
  hook_ids: [],
  shortcuts: [],
};

const published = {
  app_id: 'app-1',
  home: {
    id: 'home-1',
    app_id: 'app-1',
    hash: 'h1',
    type: 'home',
    blocks: [
      { type: 'header', text: { type: 'plain_text', text: 'Your requests' } },
      {
        type: 'actions',
        block_id: 'home',
        elements: [
          { type: 'button', action_id: 'new_request', text: { type: 'plain_text', text: 'New request' } },
        ],
      },
      { type: 'section', text: { type: 'mrkdwn', text: 'Laptop: waiting' } },
    ],
  },
};

describe('AppHomeTab', () => {
  beforeEach(() => {
    home.tab = { isPending: false, isError: false, data: published, refetch: vi.fn() };
    home.opened.mockReset();
    home.act.mockReset();
  });

  it('tells the app it was opened, from this conversation', () => {
    render(<AppHomeTab app={helpdesk} channelId="dm-1" />);
    expect(home.opened).toHaveBeenCalledWith('dm-1');
  });

  it('draws what the app published, and sends its buttons to the app', async () => {
    render(<AppHomeTab app={helpdesk} channelId="dm-1" />);

    expect(screen.getByRole('heading', { name: 'Your requests' })).toBeInTheDocument();
    expect(screen.getByText('Laptop: waiting')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'New request' }));

    await waitFor(() =>
      expect(home.act).toHaveBeenCalledWith(
        { blockId: 'home', actionId: 'new_request', value: undefined },
        expect.anything(),
      ),
    );
  });

  it('keeps its buttons dead while the app has interactivity off', () => {
    render(<AppHomeTab app={{ ...helpdesk, interactive: false }} channelId="dm-1" />);

    expect(screen.getByRole('button', { name: 'New request' })).toHaveAttribute('aria-disabled', 'true');
  });

  it('shows the app and its description until it publishes a Home', () => {
    home.tab = { isPending: false, isError: false, data: { app_id: 'app-1', home: null }, refetch: vi.fn() };
    render(<AppHomeTab app={helpdesk} channelId="dm-1" />);

    const empty = screen.getByTestId('app-home-empty');
    expect(empty).toHaveTextContent('Helpdesk');
    expect(empty).toHaveTextContent('Ask the help desk for anything');
    expect(empty).toHaveTextContent("Helpdesk hasn't set up its Home tab yet.");
  });
});
