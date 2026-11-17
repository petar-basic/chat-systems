import { describe, it, expect, vi } from 'vitest';
import { render, screen, within } from '@testing-library/react';
import { BlockMessage } from './BlockMessage';
import { everyElement, githubActions, grafana, jenkins, laidOut, sentry } from './fixtures';

vi.mock('@/features/instance/hooks/useAuth', () => ({
  useCurrentUser: () => ({ data: { id: 'u-self' } }),
}));

const INERT = 'This app has not set up interactivity';

describe('BlockMessage', () => {
  it('shows a GitHub Actions run as its header, fields and a button that opens the run', () => {
    render(<BlockMessage message={githubActions} />);
    expect(screen.getByTestId('block-header')).toHaveTextContent('✅ Build passed');
    const fields = screen.getByTestId('block-fields');
    expect(fields.children).toHaveLength(4);
    expect(within(fields).getByRole('link', { name: 'acme/api' })).toHaveAttribute(
      'href',
      'https://github.com/acme/api',
    );
    expect(fields.children[1].querySelector('br')).toBeInTheDocument();
    expect(screen.getByTestId('block-divider')).toBeInTheDocument();
    const run = screen.getByRole('link', { name: 'View run' });
    expect(run).toHaveAttribute('href', 'https://github.com/acme/api/actions/runs/9201');
    expect(run).toHaveAttribute('target', '_blank');
    expect(run).toHaveAttribute('rel', 'noopener noreferrer nofollow');
    expect(screen.queryByText('GitHub Action build result: success')).not.toBeInTheDocument();
  });

  it('shows a Grafana alert as a coloured bar with its title link and time', () => {
    render(<BlockMessage message={grafana} />);
    const bar = screen.getByTestId('legacy-attachment-bar');
    expect(bar).toHaveAttribute('data-colour', '#d63232');
    expect(screen.getByRole('link', { name: '[FIRING:1] High CPU (api-1)' })).toHaveAttribute(
      'href',
      'https://grafana.example.com/alerting/list',
    );
    expect(within(bar).getByText('Firing').closest('strong')).not.toBeNull();
    expect(within(bar).getAllByRole('listitem')).toHaveLength(2);
    const footer = screen.getByTestId('legacy-footer');
    expect(footer).toHaveTextContent('Grafana v10.4.1');
    expect(footer.querySelector('time')).toHaveAttribute('datetime', '2023-11-14T22:13:20.000Z');
    expect(footer.querySelector('time')).toHaveTextContent('2023');
    expect(screen.getAllByTestId('legacy-attachment')).toHaveLength(1);
  });

  it('shows a Sentry issue with short fields side by side and a linked footer', () => {
    render(<BlockMessage message={sentry} />);
    const fields = screen.getByTestId('legacy-fields');
    expect(within(fields).getByText('Environment')).toBeInTheDocument();
    expect(within(fields).getByText('production')).toBeInTheDocument();
    expect(fields.querySelectorAll('.col-span-2')).toHaveLength(0);
    expect(screen.getByRole('link', { name: 'BACKEND-3QZ' })).toHaveAttribute(
      'href',
      'https://sentry.io/organizations/acme/issues/4512/',
    );
  });

  it('shows a Jenkins build with Slack’s named colour and no repeat of its flattened text', () => {
    render(<BlockMessage message={jenkins} />);
    expect(screen.getByTestId('legacy-attachment-bar')).toHaveAttribute('data-colour', '#2eb886');
    expect(screen.getAllByRole('link', { name: 'Open' })).toHaveLength(1);
  });

  it('shows the text above attachments when the app sent its own', () => {
    render(
      <BlockMessage
        message={laidOut('Nightly finished', {
          legacy_attachments: [
            { color: 'javascript:alert(1)', text: 'three failed', fields: [], blocks: [] },
          ],
        })}
      />,
    );
    expect(screen.getByText('Nightly finished')).toBeInTheDocument();
    const bar = screen.getByTestId('legacy-attachment-bar');
    expect(bar).not.toHaveAttribute('data-colour');
    expect(bar.getAttribute('style')).toBeNull();
  });

  it('shows every element, and every one but a link button is inert with a reason', () => {
    render(<BlockMessage message={everyElement} />);
    expect(screen.getByAltText('Release logo')).toHaveAttribute('referrerpolicy', 'no-referrer');
    const actions = screen.getByTestId('block-actions');
    const controls = within(actions).getAllByRole('button');
    expect(controls).toHaveLength(7);
    for (const control of controls) {
      expect(control).toHaveAttribute('aria-disabled', 'true');
      expect(control).toHaveAttribute('data-tooltip', INERT);
    }
    expect(within(actions).getByRole('button', { name: 'Approve' })).toHaveClass('bg-emerald-700');
    expect(within(actions).getByRole('button', { name: 'Reject' })).toHaveClass('bg-red-700');
    const selects = within(actions).getAllByTestId('block-select');
    expect(selects.map((select) => select.textContent)).toEqual([
      'Roll out to',
      'Reviewer',
      'Select a conversation',
      '2026-11-20',
    ]);
    expect(within(actions).getByRole('button', { name: 'More options' })).toBeInTheDocument();
    const context = screen.getByTestId('block-context');
    expect(context).toHaveTextContent('Asked by Maya2 minutes ago');
    expect(within(context).getByAltText('Maya')).toBeInTheDocument();
    const image = screen.getByTestId('block-image');
    expect(image).toHaveTextContent('Error rate, last hour');
    expect(within(image).getByAltText('Error rate')).toHaveAttribute(
      'src',
      'https://cdn.example.com/chart.png',
    );
  });

  it('never links a button whose url is not the web', () => {
    render(
      <BlockMessage
        message={laidOut('', {
          blocks: [
            {
              type: 'actions',
              elements: [
                { type: 'button', text: { type: 'plain_text', text: 'Run' }, url: 'javascript:alert(1)' },
              ],
            },
          ],
        })}
      />,
    );
    expect(screen.queryByRole('link')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Run' })).toHaveAttribute('aria-disabled', 'true');
  });

  it('keeps a plain text block as text, not markup', () => {
    render(
      <BlockMessage
        message={laidOut('', {
          blocks: [{ type: 'section', text: { type: 'plain_text', text: '**not bold**' } }],
        })}
      />,
    );
    expect(screen.getByText('**not bold**')).toBeInTheDocument();
    expect(document.querySelector('strong')).toBeNull();
  });
});
