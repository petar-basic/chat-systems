import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
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

describe('SearchFilterBar in Japanese', () => {
  const hostile = '<img src=x onerror="alert(1)"> {{name}}';
  beforeEach(async () => {
    await i18n.changeLanguage('ja');
  });
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('names the filters and what they hold, with names as text', () => {
    const named: FilterContext = {
      ...context,
      members: [...context.members, { user_id: 'u-x', display_name: hostile }],
      channels: [{ id: 'c-x', name: hostile, channel_type: 'public' }],
    };
    render(
      <SearchFilterBar
        filters={{ from: 'u-x', in: 'c-x', has: 'link' }}
        onChange={vi.fn()}
        context={named}
        tab="messages"
      />,
    );
    expect(screen.getByTestId('search-filter-from').textContent).toBe(`送信者${hostile}`);
    expect(screen.getByTestId('search-filter-in').textContent).toBe(`場所#${hostile}`);
    expect(screen.getByTestId('search-filter-has').textContent).toBe('含むものリンク');
    expect(screen.getByTestId('search-filter-date').textContent).toBe('日付');
    expect(screen.getByRole('button', { name: '「送信者」の絞り込みを解除' })).toBeInTheDocument();
    expect(document.querySelector('[onerror]')).toBeNull();
  });

  it('offers dates and people in Japanese', async () => {
    render(<SearchFilterBar filters={{}} onChange={vi.fn()} context={context} tab="messages" />);
    fireEvent.click(screen.getByTestId('search-filter-date'));
    for (const label of ['今日', '昨日', '過去 7 日間', '過去 30 日間']) {
      expect(await screen.findByRole('button', { name: label })).toBeInTheDocument();
    }
    expect(screen.getByLabelText('この日より後')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('search-filter-from'));
    fireEvent.change(await screen.findByLabelText('メンバーを探す'), { target: { value: 'nobody-here' } });
    expect(screen.getByText('該当するメンバーがいません')).toBeInTheDocument();
  });
});
