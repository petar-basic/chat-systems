import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { PreferencesModal } from './PreferencesModal';

vi.mock('@/components/PushNotificationsPanel', () => ({ default: () => <div data-qa="push-panel" /> }));
vi.mock('@/components/EmailNotificationsPanel', () => ({ default: () => <div data-qa="email-panel" /> }));
vi.mock('@/components/ThemePicker', () => ({ default: () => <div data-qa="theme-panel" /> }));
vi.mock('@/features/notifications', () => ({ NotifyDefaultPanel: () => <div data-qa="notify-default" /> }));
vi.mock('@/components/TwoFactorPanel', () => ({ default: () => <div data-qa="security-panel" /> }));

const selected = () => screen.getByRole('tab', { selected: true });

describe('PreferencesModal', () => {
  it('shows one section at a time, chosen from the list', () => {
    render(<PreferencesModal onClose={vi.fn()} />);

    expect(selected()).toHaveTextContent('Notifications');
    expect(screen.getByRole('tabpanel', { name: 'Notifications' })).toContainElement(
      screen.getByTestId('email-panel'),
    );
    expect(screen.queryByTestId('theme-panel')).toBeNull();

    fireEvent.click(screen.getByRole('tab', { name: 'Security' }));
    expect(screen.getByTestId('security-panel')).toBeInTheDocument();
    expect(screen.queryByTestId('email-panel')).toBeNull();
  });

  it('moves between sections with the arrow keys, Home and End', () => {
    render(<PreferencesModal initial="appearance" onClose={vi.fn()} />);
    const list = screen.getByRole('tablist', { name: 'Preferences' });

    expect(selected()).toHaveTextContent('Appearance');
    expect(selected()).toHaveAttribute('tabindex', '0');

    fireEvent.keyDown(list, { key: 'ArrowDown' });
    expect(selected()).toHaveTextContent('Security');
    expect(selected()).toHaveFocus();

    fireEvent.keyDown(list, { key: 'ArrowDown' });
    expect(selected()).toHaveTextContent('Notifications');

    fireEvent.keyDown(list, { key: 'End' });
    expect(selected()).toHaveTextContent('Security');

    fireEvent.keyDown(list, { key: 'Home' });
    expect(selected()).toHaveTextContent('Notifications');
    expect(screen.getByTestId('push-panel')).toBeInTheDocument();
  });
});
