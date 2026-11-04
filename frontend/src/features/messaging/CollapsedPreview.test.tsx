import { describe, it, expect, beforeEach } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { CollapsedPreview } from './CollapsedPreview';
import { useDisplayStore } from '@/stores/display';

function renderPreview() {
  render(
    <CollapsedPreview label="Show 2 link previews" dataQa="collapsed">
      <div data-qa="preview">preview</div>
    </CollapsedPreview>,
  );
}

describe('CollapsedPreview', () => {
  beforeEach(() => {
    useDisplayStore.setState({ density: 'clean', previews: 'shown' });
  });

  it('shows the preview when previews are shown', () => {
    renderPreview();
    expect(screen.getByTestId('preview')).toBeInTheDocument();
    expect(screen.queryByTestId('collapsed')).not.toBeInTheDocument();
  });

  it('keeps the preview behind a line until it is clicked', () => {
    useDisplayStore.setState({ previews: 'collapsed' });
    renderPreview();

    expect(screen.queryByTestId('preview')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Show 2 link previews' }));
    expect(screen.getByTestId('preview')).toBeInTheDocument();
  });
});
