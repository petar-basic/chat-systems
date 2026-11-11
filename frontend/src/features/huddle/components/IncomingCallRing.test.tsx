import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useUserCache } from '@/stores/users';
import { IncomingCallRing } from './IncomingCallRing';

vi.mock('@/lib/notifications', () => ({ playNotificationSound: vi.fn() }));

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

describe('IncomingCallRing in Japanese', () => {
  beforeEach(async () => {
    useUserCache.setState({ users: new Map([['u-x', { id: 'u-x', display_name: hostile } as never]]) });
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the caller as text and offers to join or decline', () => {
    render(
      <IncomingCallRing
        call={{
          huddleId: 'h-1',
          fromUserId: 'u-x',
          workspaceId: 'ws-1',
          instanceUrl: 'http://localhost:3000',
        }}
      />,
    );
    expect(screen.getByRole('heading', { name: '通話の着信' })).toBeInTheDocument();
    expect(screen.getByText(hostile)).toBeInTheDocument();
    expect(screen.getByText('通話を始めようとしています…')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '拒否' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '参加' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
