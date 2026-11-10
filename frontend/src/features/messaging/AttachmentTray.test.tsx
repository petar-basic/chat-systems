import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { AttachmentTray } from './AttachmentTray';
import type { PendingAttachment } from './useComposerAttachments';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const item = (localId: string, status: PendingAttachment['status']): PendingAttachment => ({
  localId,
  name: `${hostile}.docx`,
  size: 1536 * 1024,
  mimeType: 'application/octet-stream',
  previewUrl: null,
  progress: 0.5,
  status,
  error: null,
  fileId: null,
  url: null,
  width: null,
  height: null,
});

describe('AttachmentTray in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the kind and each control after the file, as text', async () => {
    await i18n.changeLanguage('ja');
    render(
      <AttachmentTray
        items={[item('a', 'uploading'), item('b', 'failed')]}
        onRemove={vi.fn()}
        onRetry={vi.fn()}
      />,
    );
    const name = `${hostile}.docx`;
    expect(screen.getAllByText('文書 · 1.5 MB')).toHaveLength(2);
    expect(screen.getByLabelText(`${name} をアップロード中`)).toBeInTheDocument();
    const retry = screen.getByRole('button', { name: `${name} のアップロードを再試行` });
    expect(retry).toHaveAttribute('data-tooltip', 'アップロードに失敗しました');
    expect(screen.getAllByRole('button', { name: `${name} を外す` })).toHaveLength(2);
    expect(document.querySelector('[onerror]')).toBeNull();
  });
});
