import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { FormAnswers } from './FormAnswers';

vi.mock('@/components/RichTextDisplay', () => ({
  default: ({ content }: { content: string }) => <span>{content}</span>,
}));

describe('FormAnswers', () => {
  it('lays the answers out as labelled fields, a long one across both columns', () => {
    render(
      <FormAnswers
        fields={[
          { label: 'Kind', value: 'Sick' },
          { label: 'Reason', value: 'x'.repeat(80) },
          { label: 'From', value: '' },
        ]}
      />,
    );
    const terms = screen.getAllByRole('term').map((t) => t.textContent);
    expect(terms).toEqual(['Kind', 'Reason', 'From']);
    expect(screen.getByText('Sick')).toBeInTheDocument();
    expect(screen.getByText('x'.repeat(80)).closest('div')).toHaveClass('sm:col-span-2');
    expect(screen.getByText('—')).toBeInTheDocument();
  });
});
