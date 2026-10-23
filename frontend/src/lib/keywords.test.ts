import { describe, it, expect } from 'vitest';
import { foldKeyword, keywordProblem, keywordRanges } from './keywords';

const matched = (text: string, keyword: string) =>
  keywordRanges(text, [foldKeyword(keyword)]).map(([start, end]) => text.slice(start, end));

describe('keywordRanges', () => {
  it('matches whole words whatever the case or accents', () => {
    expect(matched('Deploy!', 'deploy')).toEqual(['Deploy']);
    expect(matched('deploý tonight', 'deploy')).toEqual(['deploý']);
    expect(matched('vidimo se u Čačku, u Čačak', 'cacak')).toEqual(['Čačak']);
    expect(matched('we redeployed it', 'deploy')).toEqual([]);
    expect(matched('deploy_script', 'deploy')).toEqual([]);
  });

  it('takes regex characters in a keyword literally', () => {
    expect(matched('I love C++.', 'c++')).toEqual(['C++']);
    expect(matched('a.b and axb', 'a.b')).toEqual(['a.b']);
  });

  it('keeps the original text when a letter is written decomposed', () => {
    expect(matched('éclair is here', 'eclair')).toEqual(['éclair']);
  });
});

describe('keywordProblem', () => {
  it('refuses what the server would refuse, before it is sent', () => {
    expect(keywordProblem('a', [])).toMatch(/2 to 64/);
    expect(keywordProblem('@ana', [])).toMatch(/@ or #/);
    expect(keywordProblem('#general', [])).toMatch(/@ or #/);
    expect(keywordProblem('Deplóy', ['deploy'])).toMatch(/already on your list/);
    expect(
      keywordProblem(
        'outage',
        Array.from({ length: 50 }, (_, i) => `word${i}`),
      ),
    ).toMatch(/up to 50/);
    expect(keywordProblem('outage', ['deploy'])).toBeNull();
  });
});
