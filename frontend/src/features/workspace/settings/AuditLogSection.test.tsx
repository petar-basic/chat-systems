import { describe, it, expect, vi, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen } from '@testing-library/react';
import { AuditLogSection } from './AuditLogSection';

vi.mock('@/features/workspace/hooks/useAuditLog', () => ({
  useAuditLog: () => ({ data: undefined, isLoading: false, error: {} }),
}));

describe('AuditLogSection in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says the log could not be loaded when the server gives no message', async () => {
    await i18n.changeLanguage('ja');
    render(<AuditLogSection workspaceId="ws" />);
    expect(screen.getByTestId('audit-log-panel')).toHaveTextContent('監査ログを読み込めませんでした');
  });
});
