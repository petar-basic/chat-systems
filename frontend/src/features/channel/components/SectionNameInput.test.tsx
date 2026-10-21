import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SectionNameInput } from './SectionNameInput';

describe('SectionNameInput', () => {
  it('submits once even when Enter is followed by a blur', () => {
    const onSubmit = vi.fn();
    render(<SectionNameInput label="Section name" onSubmit={onSubmit} onCancel={vi.fn()} />);
    const input = screen.getByTestId('sidebar-section-name-input');
    fireEvent.change(input, { target: { value: '  Clients ' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    fireEvent.blur(input);
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith('Clients');
  });

  it('cancels on Escape and on an empty name', () => {
    const onCancel = vi.fn();
    const onSubmit = vi.fn();
    render(<SectionNameInput label="Section name" onSubmit={onSubmit} onCancel={onCancel} />);
    fireEvent.keyDown(screen.getByTestId('sidebar-section-name-input'), { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
