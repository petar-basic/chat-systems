import { describe, it, expect } from 'vitest';
import { introOf, isLongAnswer } from './answerFields';

const fields = [
  { label: 'Reason', value: 'Dentist' },
  { label: 'Cover', value: '@[Ana](u-1)' },
  { label: 'From', value: '' },
];

describe('introOf', () => {
  it('splits the summary the workflow appended off its text', () => {
    const content =
      '@[Bob](u-2) is off\n\n**Reason**  \nDentist\n\n**Cover**  \n@[Ana](u-1)\n\n**From**  \n—';
    expect(introOf(content, fields)).toBe('@[Bob](u-2) is off');
  });

  it('gives an empty intro when the step had no text', () => {
    expect(introOf('\n\n**Reason**  \nDentist', [{ label: 'Reason', value: 'Dentist' }])).toBe('');
  });

  it('keeps line breaks of a long answer in step with the summary', () => {
    const content = 'Note\n\n**Why**  \nfirst  \nsecond';
    expect(introOf(content, [{ label: 'Why', value: 'first\nsecond' }])).toBe('Note');
  });

  it('refuses content that does not end in the summary, so the message renders as it is', () => {
    expect(introOf('Someone rewrote this', fields)).toBeNull();
    expect(introOf('anything', [])).toBeNull();
  });
});

describe('isLongAnswer', () => {
  it('spans both columns for a long or multi-line answer', () => {
    expect(isLongAnswer('Sick')).toBe(false);
    expect(isLongAnswer('x'.repeat(61))).toBe(true);
    expect(isLongAnswer('a\nb')).toBe(true);
  });
});
