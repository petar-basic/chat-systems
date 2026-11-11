import { describe, it, expect, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { displayNameOf, knownPeople, matchesPerson } from './userHelpers';
import type { WorkspaceMember } from '@/stores/workspace';

const cleo = { display_name: 'Cleo Park', email: 'cleo@dev.local', title: 'Head of Finance' };
const bob = { display_name: null, email: 'bob@dev.local', title: null };

describe('matchesPerson', () => {
  it('finds the people who have a title, whatever its case', () => {
    expect([cleo, bob].filter((p) => matchesPerson(p, ' FINANCE '))).toEqual([cleo]);
  });

  it('still finds a person by name, or by email when they have no name', () => {
    expect(matchesPerson(cleo, 'park')).toBe(true);
    expect(matchesPerson(bob, 'bob@')).toBe(true);
    expect(matchesPerson(cleo, 'cleo@')).toBe(false);
  });

  it('lets everyone through an empty search', () => {
    expect([cleo, bob].filter((p) => matchesPerson(p, '  '))).toEqual([cleo, bob]);
  });
});

describe('displayNameOf in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('calls someone without a name 不明なユーザー', async () => {
    await i18n.changeLanguage('ja');
    expect(displayNameOf('  ')).toBe('不明なユーザー');
  });
});

describe('knownPeople', () => {
  const ana = {
    user_id: 'u-ana',
    email: 'ana@dev.local',
    display_name: 'Ana',
    avatar_url: null,
    is_bot: false,
  } as WorkspaceMember;

  it('names someone who left by the name and photo they had, and marks them gone', () => {
    const people = knownPeople(
      [ana],
      [{ user_id: 'u-lena', display_name: 'Lena from Design', avatar_url: '/p.png' }],
    );

    expect(people).toContainEqual({
      id: 'u-lena',
      email: '',
      display_name: 'Lena from Design',
      avatar_url: '/p.png',
      left: true,
    });
    expect(people.find((p) => p.id === 'u-ana')?.left).toBeUndefined();
  });

  it('treats someone who came back as a member, not as gone', () => {
    const people = knownPeople([ana], [{ user_id: 'u-ana', display_name: 'Old Ana', avatar_url: null }]);

    expect(people).toHaveLength(1);
    expect(people[0]).toMatchObject({ id: 'u-ana', display_name: 'Ana', email: 'ana@dev.local' });
    expect(people[0].left).toBeUndefined();
  });
});
