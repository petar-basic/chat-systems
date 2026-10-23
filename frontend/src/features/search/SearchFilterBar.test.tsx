import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SearchFilterBar } from './SearchFilterBar';
import type { FilterContext, SearchFilters, SearchTab } from './searchFilters';

const context: FilterContext = {
  members: [
    { user_id: 'u-me', display_name: 'Petar' },
    { user_id: 'u-ana', display_name: 'Ana' },
  ],
  channels: [{ id: 'c-design', name: 'design', channel_type: 'public' }],
  conversations: [{ id: 'd-ana', kind: 'dm', participant_ids: ['u-me', 'u-ana'] }],
  currentUserId: 'u-me',
  today: new Date(2026, 9, 22),
};

function renderBar(filters: SearchFilters = {}, tab: SearchTab = 'messages') {
  const onChange = vi.fn();
  render(<SearchFilterBar filters={filters} onChange={onChange} context={context} tab={tab} />);
  return onChange;
}

describe('SearchFilterBar', () => {
  it('offers the four filters', () => {
    renderBar();
    for (const chip of ['from', 'in', 'has', 'date']) {
      expect(screen.getByTestId(`search-filter-${chip}`)).toBeInTheDocument();
    }
  });

  it('picks a person', () => {
    const onChange = renderBar();
    fireEvent.click(screen.getByTestId('search-filter-from'));
    fireEvent.change(screen.getByLabelText('Find a person'), { target: { value: 'an' } });
    fireEvent.click(screen.getByText('Ana'));
    expect(onChange).toHaveBeenCalledWith({ from: 'u-ana' });
    expect(screen.queryByTestId('search-filter-from-menu')).not.toBeInTheDocument();
  });

  it('picks a channel or a DM', () => {
    const onChange = renderBar({ has: 'file' });
    fireEvent.click(screen.getByTestId('search-filter-in'));
    fireEvent.click(screen.getByText('Ana'));
    expect(onChange).toHaveBeenCalledWith({ has: 'file', in: 'd-ana' });
  });

  it('picks what a message has', () => {
    const onChange = renderBar();
    fireEvent.click(screen.getByTestId('search-filter-has'));
    fireEvent.click(screen.getByText('Links'));
    expect(onChange).toHaveBeenCalledWith({ has: 'link' });
  });

  it('on the Files tab picks a type instead of what a message has', () => {
    const onChange = renderBar({ has: 'link' }, 'files');
    expect(screen.queryByTestId('search-filter-has')).toBeNull();
    fireEvent.click(screen.getByTestId('search-filter-type'));
    fireEvent.click(screen.getByText('Spreadsheet'));
    expect(onChange).toHaveBeenCalledWith({ has: 'link', kind: 'spreadsheet' });
  });

  it('picks a date preset and a custom range', () => {
    const onChange = renderBar();
    fireEvent.click(screen.getByTestId('search-filter-date'));
    fireEvent.click(screen.getByText('Yesterday'));
    expect(onChange).toHaveBeenLastCalledWith({ on: '2026-10-21', after: undefined, before: undefined });
    fireEvent.click(screen.getByTestId('search-filter-date'));
    fireEvent.change(screen.getByTestId('search-filter-date-after'), { target: { value: '2026-10-01' } });
    expect(onChange).toHaveBeenLastCalledWith({ on: undefined, after: '2026-10-01', before: undefined });
    expect(screen.getByTestId('search-filter-date-menu')).toBeInTheDocument();
  });

  it('shows what is set and takes it off', () => {
    const onChange = renderBar({ from: 'u-ana', in: 'c-design', has: 'image' });
    expect(screen.getByTestId('search-filter-from')).toHaveTextContent('Ana');
    expect(screen.getByTestId('search-filter-in')).toHaveTextContent('#design');
    expect(screen.getByTestId('search-filter-has')).toHaveTextContent('Images');
    fireEvent.click(screen.getByLabelText('Remove the in filter'));
    expect(onChange).toHaveBeenCalledWith({ from: 'u-ana', in: undefined, has: 'image' });
  });
});
