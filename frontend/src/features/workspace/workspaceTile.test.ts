import { describe, it, expect } from 'vitest';
import { workspaceInitials } from './workspaceTile';

describe('workspaceInitials', () => {
  it('takes the first letters of the first two words', () => {
    expect(workspaceInitials('Dev Team')).toBe('DT');
    expect(workspaceInitials('  design   ops  crew ')).toBe('DO');
  });

  it('takes two letters of a single word, so Design and Data differ', () => {
    expect(workspaceInitials('Design')).toBe('De');
    expect(workspaceInitials('data')).toBe('Da');
    expect(workspaceInitials('X')).toBe('X');
  });

  it('keeps a character that is more than one code unit whole', () => {
    expect(workspaceInitials('🚀 Launch')).toBe('🚀L');
  });

  it('falls back for an empty name', () => {
    expect(workspaceInitials('   ')).toBe('?');
  });
});
