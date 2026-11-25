import { afterEach, describe, expect, it } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter, useLocation } from 'react-router';
import { useInstanceStore, type InstanceConfig } from '@/stores/instances';
import { AppLinks } from './AppLinks';

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

function Where() {
  const location = useLocation();
  return <p data-qa="where">{`${location.pathname}${location.search}`}</p>;
}

function renderWith(href: string) {
  render(
    <MemoryRouter initialEntries={['/app/ws-1/ch-1']}>
      <AppLinks />
      <Where />
      <a href={href} target="_blank" rel="noopener noreferrer">
        link
      </a>
    </MemoryRouter>,
  );
  return screen.getByText('link');
}

function leftToBrowser(link: HTMLElement, init?: MouseEventInit): boolean {
  let left = false;
  const seen = (event: Event) => {
    left = !event.defaultPrevented;
    event.preventDefault();
  };
  window.addEventListener('click', seen);
  fireEvent.click(link, init);
  window.removeEventListener('click', seen);
  return left;
}

describe('links in messages', () => {
  afterEach(() => {
    useInstanceStore.setState({ instances: [], activeInstanceUrl: null });
  });

  it('open a message on a signed-in server in the app, on that server', () => {
    useInstanceStore.setState({
      instances: [signedIn('https://chat.example'), signedIn('https://acme.example')],
      activeInstanceUrl: 'https://chat.example',
    });
    const link = renderWith('https://acme.example/app/ws-9/ch-2/msg-3?thread=t-1');
    expect(leftToBrowser(link)).toBe(false);
    expect(screen.getByTestId('where').textContent).toBe('/app/ws-9/ch-2/msg-3?thread=t-1');
    expect(useInstanceStore.getState().activeInstanceUrl).toBe('https://acme.example');
  });

  it('leave a link to another server to the browser', () => {
    useInstanceStore.setState({ instances: [signedIn('https://chat.example')] });
    const link = renderWith('https://elsewhere.example/app/ws-9');
    expect(leftToBrowser(link)).toBe(true);
    expect(screen.getByTestId('where').textContent).toBe('/app/ws-1/ch-1');
  });

  it('leave a click with a modifier key to the browser', () => {
    useInstanceStore.setState({ instances: [signedIn('https://chat.example')] });
    const link = renderWith('https://chat.example/app/ws-1/ch-2');
    expect(leftToBrowser(link, { metaKey: true })).toBe(true);
    expect(screen.getByTestId('where').textContent).toBe('/app/ws-1/ch-1');
  });
});
