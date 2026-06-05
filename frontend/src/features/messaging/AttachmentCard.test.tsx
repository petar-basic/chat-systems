import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AttachmentCard } from './AttachmentCard';

describe('AttachmentCard', () => {
  it('renders an inline image preview for images', () => {
    render(<AttachmentCard filename="pic.png" url="/files/ws/pic.png" isImage />);
    const img = screen.getByRole('img');
    expect(img).toHaveAttribute('src', '/files/ws/pic.png');
    expect(img).toHaveAttribute('alt', 'pic.png');
  });

  it('renders a downloadable file card for non-images', () => {
    render(<AttachmentCard filename="report.pdf" url="/files/ws/report.pdf" isImage={false} />);
    expect(screen.getByText('report.pdf')).toBeInTheDocument();
    const link = screen.getByRole('link');
    expect(link).toHaveAttribute('href', '/files/ws/report.pdf');
    expect(link).toHaveAttribute('download', 'report.pdf');
  });
});
