import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter, useLocation, useNavigate } from 'react-router';
import { getCurrent } from '@tauri-apps/plugin-deep-link';
import { useInstanceStore, type InstanceConfig } from '@/stores/instances';
import { AppLinks } from './AppLinks';

const deepLinks = vi.hoisted(() => ({
  current: null as string[] | null,
  listeners: [] as ((links: string[]) => void)[],
}));

vi.mock('@/lib/phoneApp', () => ({ inPhoneApp: true }));
vi.mock('@tauri-apps/plugin-deep-link', () => ({
  getCurrent: vi.fn(async () => deepLinks.current),
  onOpenUrl: vi.fn(async (listener: (links: string[]) => void) => {
    deepLinks.listeners.push(listener);
    return () => {
      deepLinks.listeners = deepLinks.listeners.filter((l) => l !== listener);
    };
  }),
}));

function signedIn(url: string): InstanceConfig {
  return {
    url,
    user: {
      id: 'u-ana',
      email: 'ana@chat.example',
      display_name: 'Ana',
      avatar_url: null,
      instance_role: 'member',
    },
  };
}

function appLink(href: string): string {
  return `chat-systems://open?url=${encodeURIComponent(href)}`;
}

function Where() {
  const location = useLocation();
  const navigate = useNavigate();
  const server = (location.state as { server?: string } | null)?.server ?? '';
  return (
    <>
      <p data-qa="where">{`${location.pathname}${server ? ` ${server}` : ''}`}</p>
      <button type="button" onClick={() => void navigate('/app/ws-1/ch-5')}>
        elsewhere
      </button>
    </>
  );
}

async function renderApp() {
  render(
    <MemoryRouter initialEntries={['/app/ws-1/ch-1']}>
      <AppLinks />
      <Where />
    </MemoryRouter>,
  );
  await act(async () => {});
}

describe('a chat-systems://open link on a phone', () => {
  afterEach(() => {
    deepLinks.current = null;
    deepLinks.listeners = [];
    vi.mocked(getCurrent).mockClear();
    useInstanceStore.setState({ instances: [], activeInstanceUrl: null });
  });

  it('opens the message the link that started the app carries, and only once', async () => {
    useInstanceStore.setState({ instances: [signedIn('https://chat.example')] });
    deepLinks.current = [appLink('https://chat.example/app/ws-9/ch-2/msg-3')];
    await renderApp();
    expect(screen.getByTestId('where').textContent).toBe('/app/ws-9/ch-2/msg-3');

    fireEvent.click(screen.getByText('elsewhere'));
    await act(async () => {});
    expect(screen.getByTestId('where').textContent).toBe('/app/ws-1/ch-5');
    expect(getCurrent).toHaveBeenCalledTimes(1);
  });

  it('follows a link that arrives while the app is open, on its server', async () => {
    useInstanceStore.setState({
      instances: [signedIn('https://chat.example'), signedIn('https://acme.example')],
      activeInstanceUrl: 'https://chat.example',
    });
    await renderApp();
    act(() => deepLinks.listeners.forEach((l) => l([appLink('https://acme.example/app/ws-4/c/dm-1')])));
    expect(screen.getByTestId('where').textContent).toBe('/app/ws-4/c/dm-1');
    expect(useInstanceStore.getState().activeInstanceUrl).toBe('https://acme.example');
  });

  it('offers to add a server the app is not signed in to', async () => {
    useInstanceStore.setState({ instances: [signedIn('https://chat.example')] });
    deepLinks.current = [appLink('https://elsewhere.example/app/ws-1/ch-1/msg-1')];
    await renderApp();
    expect(screen.getByTestId('where').textContent).toBe('/add-instance https://elsewhere.example');
  });
});
