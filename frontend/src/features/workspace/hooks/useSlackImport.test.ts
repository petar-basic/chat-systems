import { describe, it, expect } from 'vitest';
import { workspaceNameFrom } from './useSlackImport';

describe('workspaceNameFrom', () => {
  it('takes the workspace out of the name Slack gives the file', () => {
    expect(workspaceNameFrom('Coding Craftsmen Guild Slack export Jul 27 2026 - Aug 26 2026.zip')).toBe(
      'Coding Craftsmen Guild',
    );
  });

  it('falls back to the file name when it is not Slack-shaped', () => {
    expect(workspaceNameFrom('backup.zip')).toBe('backup');
    expect(workspaceNameFrom('acme-export')).toBe('acme-export');
  });
});
