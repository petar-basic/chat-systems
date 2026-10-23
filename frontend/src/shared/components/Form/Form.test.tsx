import { describe, it, expect, vi } from 'vitest';
import { useState } from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { TextInput } from './TextInput';
import { TextArea } from './TextArea';
import { Select } from './Select';
import { Checkbox } from './Checkbox';
import { RadioCards } from './RadioCards';

describe('TextInput', () => {
  it('is named by its label and described by its hint', () => {
    render(<TextInput label="Channel name" hint="Lowercase, no spaces" value="" onChange={() => {}} />);
    const input = screen.getByRole('textbox', { name: 'Channel name' });
    expect(input).toHaveAccessibleDescription('Lowercase, no spaces');
    expect(input).not.toHaveAttribute('aria-invalid');
  });

  it('announces an error in place of the hint', () => {
    render(
      <TextInput
        label="Channel name"
        hint="Lowercase"
        error="That name is taken."
        value=""
        onChange={() => {}}
      />,
    );
    const input = screen.getByRole('textbox', { name: 'Channel name' });
    expect(input).toHaveAttribute('aria-invalid', 'true');
    expect(input).toHaveAccessibleDescription('That name is taken.');
    expect(screen.getByRole('alert')).toHaveTextContent('That name is taken.');
    expect(screen.queryByText('Lowercase')).toBeNull();
  });

  it('counts down to its limit', () => {
    function Counted() {
      const [value, setValue] = useState('');
      return (
        <TextInput
          label="Name"
          counter
          maxLength={80}
          value={value}
          onChange={(e) => setValue(e.target.value)}
        />
      );
    }
    render(<Counted />);
    expect(screen.getByText('80')).toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox', { name: 'Name' }), { target: { value: 'design' } });
    expect(screen.getByText('74')).toBeInTheDocument();
  });

  it('keeps a hidden label for screen readers', () => {
    render(<TextInput label="Search people" labelHidden value="" onChange={() => {}} />);
    expect(screen.getByRole('textbox', { name: 'Search people' })).toBeInTheDocument();
  });
});

describe('TextArea and Select', () => {
  it('are labelled the same way', () => {
    render(
      <>
        <TextArea label="Description" optional value="" onChange={() => {}} />
        <Select label="Clear after" value="never" onChange={() => {}}>
          <option value="never">Don't clear</option>
        </Select>
      </>,
    );
    expect(screen.getByRole('textbox', { name: 'Description (optional)' })).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: 'Clear after' })).toBeInTheDocument();
  });
});

describe('Checkbox', () => {
  it('toggles and is described by its explanation', () => {
    const onChange = vi.fn();
    render(
      <Checkbox
        label="Announcement channel"
        description="Only admins can post."
        checked={false}
        onChange={onChange}
      />,
    );
    const box = screen.getByRole('checkbox', { name: 'Announcement channel' });
    expect(box).toHaveAccessibleDescription('Only admins can post.');
    fireEvent.click(box);
    expect(onChange).toHaveBeenCalledWith(true);
  });
});

describe('RadioCards', () => {
  it('is one group of radios that report the choice', () => {
    const onChange = vi.fn();
    render(
      <RadioCards
        label="Visibility"
        value="public"
        onChange={onChange}
        options={[
          { value: 'public', label: 'Public', description: 'Anyone can join' },
          { value: 'private', label: 'Private', description: 'Invite only' },
        ]}
      />,
    );
    const group = screen.getByRole('group', { name: 'Visibility' });
    const radios = screen.getAllByRole('radio');
    expect(group).toContainElement(radios[0]);
    expect(new Set(radios.map((r) => r.getAttribute('name'))).size).toBe(1);
    expect(screen.getByRole('radio', { name: 'Public' })).toBeChecked();
    expect(screen.getByRole('radio', { name: 'Private' })).toHaveAccessibleDescription('Invite only');
    fireEvent.click(screen.getByRole('radio', { name: 'Private' }));
    expect(onChange).toHaveBeenCalledWith('private');
  });
});
