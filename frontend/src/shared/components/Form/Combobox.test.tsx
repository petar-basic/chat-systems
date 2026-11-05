import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';
import { Combobox, type ComboboxOption } from './Combobox';

const cities: ComboboxOption[] = [
  { value: 'Europe/Belgrade', label: 'Belgrade', detail: 'Europe · GMT+1' },
  { value: 'Europe/Berlin', label: 'Berlin', detail: 'Europe · GMT+1' },
  { value: 'Asia/Tokyo', label: 'Tokyo', detail: 'Asia · GMT+9' },
];

function renderCombobox(onChange = vi.fn()) {
  render(
    <Combobox
      label="Time zone"
      hint="Where you are"
      value="Europe/Belgrade"
      options={cities}
      onChange={onChange}
      emptyText="No time zone matches"
    />,
  );
  return { input: screen.getByRole('combobox', { name: 'Time zone' }), onChange };
}

const names = () => screen.getAllByRole('option').map((option) => option.textContent);

describe('Combobox', () => {
  it('shows the chosen option and opens on the whole list with it selected', () => {
    const { input } = renderCombobox();
    expect(input).toHaveValue('Belgrade');
    expect(input).toHaveAccessibleDescription('Where you are');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();

    fireEvent.click(input);

    expect(input).toHaveAttribute('aria-expanded', 'true');
    expect(input).toHaveValue('');
    expect(input).toHaveAttribute('placeholder', 'Belgrade');
    expect(names()).toHaveLength(3);
    expect(screen.getByRole('option', { name: /Belgrade/ })).toHaveAttribute('aria-selected', 'true');
  });

  it('narrows the list as you type and picks with Enter', () => {
    const { input, onChange } = renderCombobox();
    fireEvent.change(input, { target: { value: 'tok' } });

    expect(names()).toEqual(['TokyoAsia · GMT+9']);

    fireEvent.keyDown(input, { key: 'Enter' });

    expect(onChange).toHaveBeenCalledWith('Asia/Tokyo');
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
  });

  it('moves with the arrow keys, wrapping round, and picks with a click', () => {
    const { input, onChange } = renderCombobox();
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    expect(input).toHaveAttribute('aria-activedescendant', screen.getByRole('option', { name: /Berlin/ }).id);
    fireEvent.keyDown(input, { key: 'ArrowUp' });
    fireEvent.keyDown(input, { key: 'ArrowUp' });
    expect(input).toHaveAttribute('aria-activedescendant', screen.getByRole('option', { name: /Tokyo/ }).id);

    fireEvent.mouseDown(screen.getByRole('option', { name: /Berlin/ }));

    expect(onChange).toHaveBeenCalledWith('Europe/Berlin');
  });

  it('says so when nothing matches', () => {
    const { input } = renderCombobox();
    fireEvent.change(input, { target: { value: 'atlantis' } });

    expect(screen.getByText('No time zone matches')).toBeInTheDocument();
    expect(screen.queryAllByRole('option')).toHaveLength(0);
  });

  it('closes on Escape without closing the dialog it sits in', () => {
    const closeDialog = vi.fn();
    function Dialog() {
      useEscapeToClose(closeDialog);
      return <Combobox label="Time zone" value="Europe/Belgrade" options={cities} onChange={() => {}} />;
    }
    render(<Dialog />);
    const input = screen.getByRole('combobox', { name: 'Time zone' });
    fireEvent.click(input);

    fireEvent.keyDown(document, { key: 'Escape' });

    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    expect(closeDialog).not.toHaveBeenCalled();
    expect(input).toHaveValue('Belgrade');
  });

  it('keeps the chosen option when it loses focus', () => {
    const { input, onChange } = renderCombobox();
    fireEvent.change(input, { target: { value: 'ber' } });
    fireEvent.blur(input);

    expect(onChange).not.toHaveBeenCalled();
    expect(input).toHaveValue('Belgrade');
  });
});
