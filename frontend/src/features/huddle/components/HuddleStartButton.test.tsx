import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { useHuddleStore } from '@/stores/huddle';
import { HuddleStartButton } from './HuddleStartButton';

vi.mock('@/features/huddle/hooks/useHuddle', () => ({
  useStartHuddle: () => ({ isPending: false, mutateAsync: vi.fn() }),
}));

describe('HuddleStartButton', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the button in Japanese', async () => {
    useHuddleStore.setState({ active: null });
    await i18n.changeLanguage('ja');
    render(
      <HuddleStartButton
        workspaceId="ws-1"
        instanceUrl="http://localhost:3000"
        partnerId="u-2"
        currentUserId="u-1"
      />,
    );
    expect(screen.getByRole('button', { name: '通話を開始' })).toHaveAttribute('data-tooltip', '通話を開始');
  });
});
