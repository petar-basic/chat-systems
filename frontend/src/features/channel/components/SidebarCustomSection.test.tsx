import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { SidebarCustomSection } from './SidebarCustomSection';

describe('SidebarCustomSection', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('in Japanese names its options after the section, as text', async () => {
    const name = '<img src=x onerror="alert(1)"> {{name}}';
    const onDelete = vi.fn();
    await i18n.changeLanguage('ja');
    render(
      <SidebarCustomSection
        id="sec-1"
        name={name}
        collapsed={false}
        onToggle={vi.fn()}
        onRename={vi.fn()}
        onDelete={onDelete}
        onMoveUp={vi.fn()}
        onMoveDown={vi.fn()}
        onDropChannel={vi.fn()}
      >
        <span>child</span>
      </SidebarCustomSection>,
    );
    const options = screen.getByRole('button', { name: `${name} の操作` });
    expect(options).toHaveAttribute('data-tooltip', `${name} の操作`);
    fireEvent.click(options);
    expect(await screen.findByRole('button', { name: '名前を変更' })).toBeInTheDocument();
    for (const label of ['上へ移動', '下へ移動']) {
      expect(screen.getByRole('button', { name: label })).toBeInTheDocument();
    }
    fireEvent.click(screen.getByRole('button', { name: 'セクションを削除' }));
    expect(onDelete).toHaveBeenCalledOnce();
    expect(document.querySelector('img')).toBeNull();
  });
});
