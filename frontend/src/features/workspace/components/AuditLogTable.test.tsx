import { describe, it, expect, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { render, screen, within } from '@testing-library/react';
import AuditLogTable, { type AuditEntry } from './AuditLogTable';

const hostile = '<img src=x onerror="alert(1)"> {{name}}';

const entry = (id: string, action: string, actor: Partial<AuditEntry> = {}): AuditEntry => ({
  id,
  action,
  created_at: '2026-11-14T09:00:00Z',
  details: null,
  ...actor,
});

function actions() {
  return screen.getAllByTestId('audit-log-action').map((cell) => cell.textContent);
}

function leaves(value: unknown, prefix = ''): string[] {
  if (typeof value !== 'object' || value === null) return [prefix];
  return Object.entries(value).flatMap(([key, child]) => leaves(child, prefix ? `${prefix}.${key}` : key));
}

describe('AuditLogTable', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names known actions and spells out the ones it does not know', () => {
    render(
      <AuditLogTable
        entries={[
          entry('1', 'channel.archived', { actor_display_name: 'Ana' }),
          entry('2', 'webhook.fired'),
          entry('3', 'channel'),
        ]}
        loading={false}
      />,
    );
    expect(actions()).toEqual(['Channel archived', 'Webhook fired', 'Channel']);
    expect(screen.getByRole('columnheader', { name: 'When' })).toBeInTheDocument();
    expect(screen.getAllByTestId('audit-log-row')[1]).toHaveTextContent('unknown');
  });

  it('says when nothing is recorded', () => {
    render(<AuditLogTable entries={[]} loading={false} />);
    expect(screen.getByTestId('audit-log-empty')).toHaveTextContent('Nothing has been recorded yet.');
  });
});

describe('AuditLogTable in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the columns and the actions, with names as text', async () => {
    await i18n.changeLanguage('ja');
    render(
      <AuditLogTable
        entries={[
          entry('1', 'channel.archived', { actor_display_name: hostile }),
          entry('2', 'totp.failed', { actor_email: 'ana@example.com' }),
          entry('3', 'webhook.fired'),
        ]}
        loading={false}
        showWorkspace
      />,
    );
    const headers = screen.getAllByRole('columnheader').map((cell) => cell.textContent);
    expect(headers).toEqual(['日時', '実行者', '操作', 'ワークスペース', '詳細', '接続元']);
    expect(actions()).toEqual(['チャンネルをアーカイブ', '2要素認証に失敗', 'Webhook fired']);
    const [first, , third] = screen.getAllByTestId('audit-log-row');
    expect(within(first).getByText(hostile)).toBeInTheDocument();
    expect(third).toHaveTextContent('不明');
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('reads only its own actions, not other strings named in the log', async () => {
    await i18n.changeLanguage('ja');
    render(
      <AuditLogTable
        entries={[entry('1', 'channel'), entry('2', 'audit.empty'), entry('3', 'common:cancel')]}
        loading={false}
      />,
    );
    expect(actions()).toEqual(['Channel', 'Audit empty', 'Common:cancel']);
  });

  it('says when nothing is recorded', async () => {
    await i18n.changeLanguage('ja');
    render(<AuditLogTable entries={[]} loading={false} />);
    expect(screen.getByTestId('audit-log-empty')).toHaveTextContent('まだ記録はありません。');
  });

  it('has Japanese for every action', () => {
    const english = leaves(i18n.getResource('en', 'admin', 'audit'));
    const japanese = leaves(i18n.getResource('ja', 'admin', 'audit'));
    expect(english).toHaveLength(97);
    expect(japanese).toEqual(english);
  });
});
