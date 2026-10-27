import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { useNotificationPrefs } from '@/stores/notificationPrefs';
import DesktopNotificationsPanel from './DesktopNotificationsPanel';

const requestPermission = vi.fn();
let permission: NotificationPermission = 'default';

describe('DesktopNotificationsPanel', () => {
  beforeEach(() => {
    permission = 'default';
    requestPermission.mockReset().mockImplementation(async () => {
      permission = 'granted';
      return permission;
    });
    vi.stubGlobal('Notification', {
      get permission() {
        return permission;
      },
      requestPermission,
    });
    useNotificationPrefs.setState({ desktopEnabled: false });
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('asks the browser before turning desktop notifications on, and turns them off again', async () => {
    render(<DesktopNotificationsPanel />);

    fireEvent.click(screen.getByRole('button', { name: 'Turn on' }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Turn off' })).toBeInTheDocument());
    expect(requestPermission).toHaveBeenCalledTimes(1);
    expect(useNotificationPrefs.getState().desktopEnabled).toBe(true);

    fireEvent.click(screen.getByRole('button', { name: 'Turn off' }));
    expect(useNotificationPrefs.getState().desktopEnabled).toBe(false);
    expect(screen.getByRole('button', { name: 'Turn on' })).toBeInTheDocument();
  });

  it('says so when the browser has blocked them', () => {
    permission = 'denied';
    render(<DesktopNotificationsPanel />);

    expect(screen.getByTestId('desktop-notifications')).toHaveTextContent(/blocked notifications/);
    expect(screen.getByRole('button', { name: 'Turn on' })).toBeDisabled();
  });
});
