import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { LinkPreviews } from './LinkPreviews';

vi.mock('@/features/instance/hooks/useAuth', () => ({
  useCurrentUser: () => ({ data: { id: 'user-self' } }),
}));

const jira = {
  url: 'https://acme.atlassian.net/browse/PT-262',
  title: 'Focus Axon planning',
  description: null,
  site_name: 'Jira',
  image_url: null,
  kind: 'app',
  provider: 'jira',
  subtitle: 'Story PT-262',
  fields: [
    { label: 'Status', value: 'In Progress', tone: 'info' },
    { label: 'Assignee', value: 'Lazar' },
  ],
  fetched_at: '2026-10-21T10:00:00Z',
};

describe('LinkPreviews', () => {
  it('renders an app card with its fields and a way to open it', () => {
    const onRefresh = vi.fn();
    render(<LinkPreviews unfurls={[jira]} onRefresh={onRefresh} />);
    const card = screen.getByTestId('app-preview');
    expect(card).toHaveTextContent('Focus Axon planning');
    expect(card).toHaveTextContent('Story PT-262');
    expect(card).toHaveTextContent('In Progress');
    expect(card).toHaveTextContent('Lazar');
    expect(screen.getByText('Open in Jira').closest('a')).toHaveAttribute('href', jira.url);
    fireEvent.click(screen.getByTestId('app-preview-refresh'));
    expect(onRefresh).toHaveBeenCalled();
  });

  it('shows the card picture and drops it when it fails to load', () => {
    render(<LinkPreviews unfurls={[{ ...jira, image_url: 'https://cdn.test/thumb.png' }]} />);
    const image = screen.getByTestId('app-preview-image');
    expect(image).toHaveAttribute('src', 'https://cdn.test/thumb.png');
    fireEvent.error(image);
    expect(screen.queryByTestId('app-preview-image')).not.toBeInTheDocument();
    expect(screen.getByTestId('app-preview')).toHaveTextContent('Focus Axon planning');
  });

  it('never shows a picture that is not https', () => {
    render(<LinkPreviews unfurls={[{ ...jira, image_url: 'http://cdn.test/thumb.png' }]} />);
    expect(screen.queryByTestId('app-preview-image')).not.toBeInTheDocument();
  });

  it('plays a video inline only when asked to', () => {
    render(
      <LinkPreviews
        unfurls={[
          {
            url: 'https://youtu.be/dQw4w9WgXcQ?t=90',
            title: 'Never Gonna Give You Up',
            description: null,
            site_name: 'YouTube',
            image_url: 'https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg',
            kind: 'video',
            subtitle: 'Rick Astley',
            embed: { provider: 'youtube', id: 'dQw4w9WgXcQ', start: 90 },
          },
        ]}
      />,
    );
    expect(screen.getByTestId('video-preview')).toHaveTextContent('Rick Astley');
    expect(screen.queryByTestId('video-player')).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId('video-play'));
    expect(screen.getByTestId('video-player')).toHaveAttribute(
      'src',
      'https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?autoplay=1&start=90',
    );
  });

  it('draws an app preview in its blocks under the app name', () => {
    const onHide = vi.fn();
    render(
      <LinkPreviews
        unfurls={[
          {
            url: 'https://tracker.example.com/issues/7',
            title: 'Tracker',
            description: null,
            site_name: 'Tracker',
            image_url: null,
            kind: 'blocks',
            app: { id: 'app-1', bot_user_id: 'bot-1', name: 'Tracker', icon_url: null },
            blocks: [
              { type: 'header', text: { type: 'plain_text', text: 'TRK-7 Login fails' } },
              {
                type: 'section',
                fields: [
                  { type: 'mrkdwn', text: '*Status*\nOpen' },
                  { type: 'mrkdwn', text: '*Assignee*\nAna' },
                ],
              },
            ],
          },
        ]}
        onHide={onHide}
      />,
    );
    const card = screen.getByTestId('app-blocks-preview');
    expect(screen.getByTestId('app-blocks-preview-app')).toHaveTextContent('Tracker');
    expect(screen.getByTestId('block-header')).toHaveTextContent('TRK-7 Login fails');
    expect(card).toHaveTextContent('Open');
    expect(card).toHaveTextContent('Ana');
    fireEvent.click(screen.getByTestId('app-blocks-preview-remove'));
    expect(onHide).toHaveBeenCalled();
  });

  it('skips an app preview without blocks', () => {
    const { container } = render(
      <LinkPreviews
        unfurls={[
          {
            url: 'https://tracker.example.com/issues/7',
            title: 'Tracker',
            description: null,
            site_name: 'Tracker',
            image_url: null,
            kind: 'blocks',
            app: { id: 'app-1', bot_user_id: 'bot-1', name: 'Tracker', icon_url: null },
            blocks: [],
          },
        ]}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('shows nothing for a page that gave only a title', () => {
    const { container } = render(
      <LinkPreviews
        unfurls={[
          { url: 'https://x.test', title: 'Jira', description: null, site_name: 'x.test', image_url: null },
        ]}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });
});

describe('LinkPreviews in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the play and remove buttons, with the title as text', async () => {
    const title = '<img src=x onerror="alert(1)"> {{name}}';
    await i18n.changeLanguage('ja');
    render(
      <LinkPreviews
        onHide={vi.fn()}
        unfurls={[
          {
            url: 'https://youtu.be/dQw4w9WgXcQ',
            title,
            description: null,
            site_name: 'YouTube',
            image_url: 'https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg',
            kind: 'video',
            subtitle: null,
            embed: { provider: 'youtube', id: 'dQw4w9WgXcQ', start: null },
          },
        ]}
      />,
    );
    expect(screen.getByTestId('video-play')).toHaveAccessibleName(`${title} を再生`);
    expect(screen.getByRole('button', { name: 'プレビューを削除' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('says when an app preview was fetched', async () => {
    await i18n.changeLanguage('ja');
    render(<LinkPreviews unfurls={[jira]} onRefresh={vi.fn()} />);
    expect(screen.getByText(/ 時点$/)).toBeInTheDocument();
  });
});
