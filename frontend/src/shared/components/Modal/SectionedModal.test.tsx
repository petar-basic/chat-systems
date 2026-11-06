import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Bell, Palette, Shield } from 'lucide-react';
import { SectionedModal, type ModalSection } from './SectionedModal';

type Key = 'notifications' | 'appearance' | 'security';

const SECTIONS: ModalSection<Key>[] = [
  { key: 'notifications', label: 'Notifications', Icon: Bell },
  { key: 'appearance', label: 'Appearance', Icon: Palette },
  { key: 'security', label: 'Security', Icon: Shield },
];

const renderModal = (active: Key) => (
  <SectionedModal
    title="Preferences"
    sections={SECTIONS}
    active={active}
    onSelect={vi.fn()}
    onClose={vi.fn()}
    qaPrefix="prefs"
  >
    <p>Body</p>
  </SectionedModal>
);

describe('SectionedModal', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('brings the chosen tab into view, on opening and on every change', () => {
    const scrolled = vi.spyOn(Element.prototype, 'scrollIntoView');
    const { rerender } = render(renderModal('security'));
    expect(scrolled.mock.contexts.at(-1)).toBe(screen.getByRole('tab', { name: 'Security' }));

    rerender(renderModal('appearance'));
    expect(scrolled.mock.contexts.at(-1)).toBe(screen.getByRole('tab', { name: 'Appearance' }));
    expect(scrolled).toHaveBeenLastCalledWith({ block: 'nearest', inline: 'nearest' });
  });
});
