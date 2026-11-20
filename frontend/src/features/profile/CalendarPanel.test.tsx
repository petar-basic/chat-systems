import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import CalendarPanel from './CalendarPanel';
import type { CalendarState } from './hooks/useCalendars';

const update = vi.fn();
const connect = vi.fn();
const disconnect = vi.fn();
let calendars: CalendarState[] = [];

vi.mock('@/stores/workspace', () => ({
  useWorkspaceStore: (select: (state: unknown) => unknown) => select({ currentWorkspace: { id: 'w-1' } }),
}));
vi.mock('@/features/workspace', () => ({
  useAppActions: () => ({
    connect: { mutate: connect, isPending: false },
    disconnect: { mutate: disconnect, isPending: false },
  }),
}));
vi.mock('./hooks/useCalendars', () => ({
  useCalendars: () => ({ data: { data: calendars }, isLoading: false, isError: false, refetch: vi.fn() }),
  useUpdateCalendar: () => ({ mutate: update }),
}));

const settings = {
  status: true,
  reminders: true,
  agenda: true,
  agenda_at: '09:00',
  last_synced_at: null,
  last_error: null,
};

const card = (provider: string) => within(screen.getByTestId(`calendar-${provider}`));

describe('CalendarPanel', () => {
  beforeEach(() => {
    update.mockReset();
    connect.mockReset();
    disconnect.mockReset();
    calendars = [];
  });

  it('offers Connect where an admin set the calendar up, and asks for an admin where not', () => {
    calendars = [
      { provider: 'google_calendar', name: 'Google Calendar', set_up: true, connected_account: null },
      { provider: 'outlook_calendar', name: 'Outlook Calendar', set_up: false, connected_account: null },
    ];
    render(<CalendarPanel />);

    expect(
      card('outlook_calendar').getByText('Ask a workspace admin to set this up in Apps.'),
    ).toBeInTheDocument();
    expect(card('outlook_calendar').queryByRole('button', { name: 'Connect' })).toBeNull();
    expect(card('google_calendar').queryByTestId('calendar-status')).toBeNull();

    fireEvent.click(card('google_calendar').getByRole('button', { name: 'Connect' }));
    expect(connect).toHaveBeenCalledWith({
      provider: 'google_calendar',
      returnTo: `${window.location.pathname}?preferences=calendar`,
    });
  });

  it('saves each setting of a connected calendar, and disconnects it', () => {
    calendars = [
      {
        provider: 'outlook_calendar',
        name: 'Outlook Calendar',
        set_up: true,
        connected_account: 'Calendar (bob@acme.test)',
        settings,
      },
    ];
    render(<CalendarPanel />);

    expect(screen.getByText('Calendar (bob@acme.test)')).toBeInTheDocument();
    expect(screen.getByTestId('calendar-sync')).toHaveTextContent('Getting your events…');

    fireEvent.click(screen.getByRole('checkbox', { name: /Show when I'm in a meeting/ }));
    expect(update).toHaveBeenLastCalledWith({ provider: 'outlook_calendar', changes: { status: false } });
    fireEvent.click(screen.getByRole('checkbox', { name: /Remind me a minute before/ }));
    expect(update).toHaveBeenLastCalledWith({ provider: 'outlook_calendar', changes: { reminders: false } });

    const at = screen.getByLabelText('Send at');
    fireEvent.change(at, { target: { value: '09:00' } });
    fireEvent.blur(at);
    expect(update).toHaveBeenCalledTimes(2);
    fireEvent.change(at, { target: { value: '07:45' } });
    fireEvent.blur(at);
    expect(update).toHaveBeenLastCalledWith({
      provider: 'outlook_calendar',
      changes: { agenda_at: '07:45' },
    });

    fireEvent.click(screen.getByRole('button', { name: 'Disconnect' }));
    expect(disconnect).toHaveBeenCalledWith('outlook_calendar');
  });

  it('hides the agenda time while the agenda is off, and says why a sync failed', () => {
    calendars = [
      {
        provider: 'google_calendar',
        name: 'Google Calendar',
        set_up: true,
        connected_account: 'ana@acme.test',
        settings: {
          ...settings,
          agenda: false,
          last_error: 'Google Calendar refused the connection; connect again',
        },
      },
    ];
    render(<CalendarPanel />);

    expect(screen.queryByLabelText('Send at')).toBeNull();
    expect(screen.getByTestId('calendar-sync')).toHaveTextContent(
      "Couldn't sync: Google Calendar refused the connection; connect again",
    );
  });
});
