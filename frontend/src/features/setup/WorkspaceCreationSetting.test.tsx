import { describe, it, expect, vi, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { fireEvent, render, screen } from '@testing-library/react';
import { WorkspaceCreationSetting } from './WorkspaceCreationSetting';

const settings = vi.hoisted(() => ({
  data: { email_code_sign_in: true, email_configured: true, workspace_creation_restricted: false },
  mutate: vi.fn(),
  isError: false,
  error: null as Error | null,
}));

vi.mock('@/features/instance', () => ({
  useInstanceSettings: () => ({ data: settings.data }),
  useUpdateInstanceSettings: () => ({
    mutate: settings.mutate,
    isPending: false,
    isError: settings.isError,
    error: settings.error,
  }),
}));

describe('WorkspaceCreationSetting', () => {
  afterEach(() => {
    settings.mutate.mockReset();
    settings.data = {
      email_code_sign_in: true,
      email_configured: true,
      workspace_creation_restricted: false,
    };
    settings.isError = false;
    settings.error = null;
  });

  it('restricts creation to workspace creators and admins, and changes nothing else', () => {
    render(<WorkspaceCreationSetting instanceUrl="https://chat.example.com" />);
    expect(screen.getByRole('radio', { name: 'Everyone' })).toBeChecked();

    fireEvent.click(screen.getByRole('radio', { name: 'Workspace creators and admins' }));
    expect(settings.mutate).toHaveBeenCalledWith({ workspace_creation_restricted: true });
  });

  it('opens it to everyone again, and does not save the choice it already has', () => {
    settings.data = { ...settings.data, workspace_creation_restricted: true };
    render(<WorkspaceCreationSetting instanceUrl="https://chat.example.com" />);
    expect(screen.getByRole('radio', { name: 'Workspace creators and admins' })).toBeChecked();

    fireEvent.click(screen.getByRole('radio', { name: 'Workspace creators and admins' }));
    expect(settings.mutate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('radio', { name: 'Everyone' }));
    expect(settings.mutate).toHaveBeenCalledWith({ workspace_creation_restricted: false });
  });

  it('says so when the setting could not be saved', () => {
    settings.isError = true;
    settings.error = new Error('Not allowed');
    render(<WorkspaceCreationSetting instanceUrl="https://chat.example.com" />);
    expect(screen.getByRole('alert')).toHaveTextContent('Not allowed');
  });
});

describe('WorkspaceCreationSetting in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names both choices', async () => {
    await i18n.changeLanguage('ja');
    render(<WorkspaceCreationSetting instanceUrl="https://chat.example.com" />);
    expect(screen.getByText('ワークスペースを作成できる人')).toBeInTheDocument();
    expect(screen.getByTestId('workspace-creation-creators')).toHaveTextContent(
      'ワークスペース作成者とサーバー管理者',
    );
    expect(screen.getByTestId('workspace-creation-everyone')).toHaveTextContent(
      'サインインできる人なら誰でも、ワークスペースを作成できます。',
    );
  });
});
