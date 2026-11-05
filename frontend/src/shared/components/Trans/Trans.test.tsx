import { describe, it, expect, beforeAll } from 'vitest';
import { render } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { Trans } from './Trans';

const HOSTILE = [
  '<img src=x onerror="alert(1)">',
  '<name>x</name>',
  '</name><img src=x>',
  '{{name}}',
  '{{name}}{{name}}',
  '{{- name}}',
  '{{defaultValue}}',
  '{{ns}}',
  '{{name',
  '}}{{',
  '$t(auth:signIn.welcome)',
  '&lt;b&gt;',
  '&#123;&#123;name&#125;&#125;',
  '&amp;',
  `'"/&`,
];

beforeAll(() => {
  i18n.addResourceBundle('en', 'transTest', { line: 'Hello <name>{{name}}</name>, welcome.' });
});

function renderLine(name: string) {
  const { container } = render(
    <Trans
      ns={'transTest' as never}
      i18nKey={'line' as never}
      values={{ name }}
      components={{ name: <strong /> }}
    />,
  );
  return container;
}

describe('Trans', () => {
  it('puts a value in its tag', () => {
    const line = renderLine('Ana');
    expect(line.textContent).toBe('Hello Ana, welcome.');
    expect(line.querySelector('strong')?.textContent).toBe('Ana');
  });

  it('shows any value exactly as it was given, as text', () => {
    for (const name of HOSTILE) {
      const line = renderLine(name);
      expect(line.textContent, name).toBe(`Hello ${name}, welcome.`);
      expect(line.querySelectorAll('strong'), name).toHaveLength(1);
      expect(line.querySelector('strong')?.textContent, name).toBe(name);
      expect(line.querySelector('img'), name).toBeNull();
    }
  });
});
