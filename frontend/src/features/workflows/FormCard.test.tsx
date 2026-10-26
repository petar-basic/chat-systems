import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { FormCard } from './FormCard';
import type { FormRequest } from './workflowModel';

const submit =
  vi.fn<(vars: { runId: string; position: number; answers: Record<string, string> }) => Promise<unknown>>();

vi.mock('./useWorkflows', () => ({
  useSubmitForm: () => ({ mutateAsync: submit, isPending: false }),
}));
vi.mock('@/features/workspace/hooks/useWorkspaces', () => ({
  useWorkspaceMembers: () => ({
    data: [
      { user_id: 'u-me', display_name: 'Me' },
      { user_id: 'u-ana', display_name: 'Ana' },
    ],
  }),
}));
vi.mock('@/stores/users', () => ({
  useUserCache: () => ({ getUser: (id: string) => (id === 'u-bob' ? { display_name: 'Bob' } : undefined) }),
}));

const formOf = (overrides: Partial<FormRequest> = {}): FormRequest => ({
  run_id: 'r-1',
  position: 0,
  title: 'Time off',
  recipient_id: 'u-me',
  expires_at: '2026-10-31T08:00:00Z',
  submitted_at: null,
  expired: false,
  fields: [
    { name: 'reason', label: 'Reason', kind: 'long_text', required: true },
    { name: 'kind', label: 'Kind', kind: 'select', options: ['Sick', 'Holiday'], required: true },
    { name: 'cover', label: 'Cover', kind: 'person' },
    { name: 'from', label: 'From', kind: 'date' },
  ],
  ...overrides,
});

const answers = () => screen.getAllByTestId('workflow-form-answer');

beforeEach(() => {
  submit.mockReset();
});

describe('FormCard', () => {
  it('opens the form from its button and sends the answers back', async () => {
    submit.mockResolvedValue({ status: 'submitted' });
    render(<FormCard form={formOf()} currentUserId="u-me" />);
    expect(screen.getByTestId('workflow-form-card-title')).toHaveTextContent('Time off');
    fireEvent.click(screen.getByTestId('workflow-form-fill'));

    const dialog = screen.getByTestId('workflow-form-dialog');
    expect(within(dialog).getByText('Reason')).toBeInTheDocument();
    expect(answers()).toHaveLength(4);
    expect(
      within(answers()[2])
        .getAllByRole('option')
        .map((o) => o.textContent),
    ).toEqual(['Pick a person…', 'Me', 'Ana']);
    expect(answers()[3]).toHaveAttribute('type', 'date');

    fireEvent.click(screen.getByTestId('workflow-form-submit'));
    expect(await screen.findByTestId('workflow-form-error')).toHaveTextContent('“Reason” needs an answer');
    expect(submit).not.toHaveBeenCalled();

    fireEvent.change(answers()[0], { target: { value: 'Dentist' } });
    fireEvent.change(answers()[1], { target: { value: 'Sick' } });
    fireEvent.change(answers()[2], { target: { value: 'u-ana' } });
    fireEvent.change(answers()[3], { target: { value: '2026-11-02' } });
    fireEvent.click(screen.getByTestId('workflow-form-submit'));

    await waitFor(() =>
      expect(submit).toHaveBeenCalledWith({
        runId: 'r-1',
        position: 0,
        answers: { reason: 'Dentist', kind: 'Sick', cover: 'u-ana', from: '2026-11-02' },
      }),
    );
    await waitFor(() => expect(screen.queryByTestId('workflow-form-dialog')).toBeNull());
    expect(screen.getByTestId('workflow-form-card-status')).toHaveTextContent('Filled in');
    expect(screen.queryByTestId('workflow-form-fill')).toBeNull();
  });

  it('keeps the dialog open with the server’s reason when a submit is refused', async () => {
    submit.mockRejectedValue(new Error('This form has expired'));
    render(
      <FormCard
        form={formOf({ fields: [{ name: 'note', label: 'Note', kind: 'short_text' }] })}
        currentUserId="u-me"
      />,
    );
    fireEvent.click(screen.getByTestId('workflow-form-fill'));
    fireEvent.click(screen.getByTestId('workflow-form-submit'));
    expect(await screen.findByTestId('workflow-form-error')).toHaveTextContent('This form has expired');
    expect(screen.getByTestId('workflow-form-dialog')).toBeInTheDocument();
  });

  it('shows everyone else who it waits for, and no button', () => {
    render(<FormCard form={formOf({ recipient_id: 'u-bob' })} currentUserId="u-me" />);
    expect(screen.getByTestId('workflow-form-card-status')).toHaveTextContent('Waiting for Bob');
    expect(screen.queryByTestId('workflow-form-fill')).toBeNull();
  });

  it('closes once it is filled in or expired', () => {
    const { rerender } = render(<FormCard form={formOf({ expired: true })} currentUserId="u-me" />);
    expect(screen.getByTestId('workflow-form-card-status')).toHaveTextContent('This form expired');
    expect(screen.queryByTestId('workflow-form-fill')).toBeNull();
    rerender(<FormCard form={formOf({ submitted_at: '2026-10-24T09:00:00Z' })} currentUserId="u-me" />);
    expect(screen.getByTestId('workflow-form-card-status')).toHaveTextContent('Filled in');
    expect(screen.queryByTestId('workflow-form-fill')).toBeNull();
  });
});
