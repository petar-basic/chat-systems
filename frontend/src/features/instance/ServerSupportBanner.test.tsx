import { beforeEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { useServerSupportStore } from '@/stores/serverSupport';
import ServerSupportBanner from './ServerSupportBanner';

describe('ServerSupportBanner', () => {
  beforeEach(() => {
    useServerSupportStore.setState({ problems: {} });
  });

  it('shows nothing while every server serves this app', () => {
    render(<ServerSupportBanner />);
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('says which server needs a newer app, and from which version', () => {
    useServerSupportStore.getState().report('https://chat.example.com', {
      kind: 'needsNewerApp',
      minVersion: '6.0.0',
    });
    render(<ServerSupportBanner />);

    expect(screen.getByRole('status')).toHaveTextContent(
      'chat.example.com needs a newer app — it serves Chat Systems 6.0.0 or later. Reload to get it, or update the app.',
    );
    expect(screen.getByRole('button', { name: 'Reload' })).toBeInTheDocument();
  });

  it('says which server is too old for this app', () => {
    useServerSupportStore.getState().report('https://old.example.com', { kind: 'serverTooOld' });
    render(<ServerSupportBanner />);

    expect(screen.getByRole('status')).toHaveTextContent(
      'old.example.com runs an older Chat Systems — this app needs 5.0 or later there. Ask its admin to update it.',
    );
    expect(screen.queryByRole('button')).toBeNull();
  });
});
