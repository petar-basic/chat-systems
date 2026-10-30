import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { WorkspaceIcon } from './WorkspaceIcon';

const box = { className: 'w-10 h-10', initialsClassName: 'bg-purple-600' };

describe('WorkspaceIcon', () => {
  it('shows the uploaded icon', () => {
    render(<WorkspaceIcon workspace={{ name: 'Acme', icon_url: '/icons/acme.png' }} {...box} />);
    expect(screen.getByTestId('workspace-icon-image')).toHaveAttribute('src', '/icons/acme.png');
  });

  it('shows initials when there is no icon', () => {
    render(<WorkspaceIcon workspace={{ name: 'Coding Guild', icon_url: null }} {...box} />);
    expect(screen.getByText('CG')).toBeInTheDocument();
  });

  it('falls back to initials when the icon fails to load', () => {
    render(<WorkspaceIcon workspace={{ name: 'Coding Guild', icon_url: '/icons/gone.png' }} {...box} />);
    fireEvent.error(screen.getByTestId('workspace-icon-image'));
    expect(screen.getByText('CG')).toBeInTheDocument();
    expect(screen.queryByTestId('workspace-icon-image')).toBeNull();
  });
});
