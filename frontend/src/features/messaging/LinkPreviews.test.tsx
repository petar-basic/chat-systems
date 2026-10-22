import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { LinkPreviews } from './LinkPreviews';

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
