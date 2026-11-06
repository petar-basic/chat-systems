import { describe, it, expect, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { CopyField } from './CopyField';

describe('CopyField', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the copy button after what it copies', () => {
    render(<CopyField label="Signing secret" value="s3cret" />);
    expect(screen.getByRole('button', { name: 'Copy Signing secret' })).toBeInTheDocument();
  });

  it('in Japanese names it as text, whatever the label says', async () => {
    const label = '<img src=x onerror="alert(1)"> {{name}}';
    await i18n.changeLanguage('ja');
    render(<CopyField label={label} value="s3cret" />);
    expect(screen.getByRole('button', { name: `${label}をコピー` })).toBeInTheDocument();
    expect(document.querySelector('img')).toBeNull();
  });
});
