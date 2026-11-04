import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { OffHoursNote } from './OffHoursNote';
import { LocalTimeLine } from './LocalTimeLine';
import { useUserCache } from '@/stores/users';
import { useDisplayStore } from '@/stores/display';

function person(timezone: string | null) {
  useUserCache.setState({
    users: new Map([
      ['u-ana', { id: 'u-ana', email: 'ana@x.test', display_name: 'Ana', avatar_url: null, timezone }],
    ]),
  });
}

describe('their local time', () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(new Date(Date.UTC(2026, 10, 4, 22, 0)));
    useDisplayStore.setState({ clock: 'h24' });
  });
  afterEach(() => vi.useRealTimers());

  it('shows on the profile card in the clock picked', () => {
    const { rerender } = render(<LocalTimeLine timezone="Asia/Tokyo" />);
    expect(screen.getByTestId('hover-card-local-time')).toHaveTextContent(/07:00 local time/);

    useDisplayStore.setState({ clock: 'h12' });
    rerender(<LocalTimeLine timezone="Asia/Tokyo" />);
    expect(screen.getByTestId('hover-card-local-time')).toHaveTextContent(/^7:00\s?AM local time/i);
  });

  it('is left off a profile without a time zone', () => {
    render(<LocalTimeLine timezone={null} />);
    expect(screen.queryByTestId('hover-card-local-time')).toBeNull();
  });

  it('says above a DM composer when it is late for them', () => {
    person('Europe/Belgrade');
    render(<OffHoursNote userId="u-ana" />);
    expect(screen.getByTestId('off-hours-note')).toHaveTextContent('It is 23:00 for Ana, late in their day.');
  });

  it('says nothing during their day', () => {
    person('America/New_York');
    render(<OffHoursNote userId="u-ana" />);
    expect(screen.queryByTestId('off-hours-note')).toBeNull();
  });
});
