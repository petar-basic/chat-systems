import { afterEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { ConsentDialog } from './ConsentDialog';

describe('ConsentDialog', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says in Japanese when an app asks for nothing', async () => {
    await i18n.changeLanguage('ja');
    render(
      <ConsentDialog
        title="Echo"
        lead="Lead"
        permissions={[]}
        confirmLabel="OK"
        onConfirm={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByText('ここに表示されること以外には、何もできません。')).toBeInTheDocument();
  });
});
