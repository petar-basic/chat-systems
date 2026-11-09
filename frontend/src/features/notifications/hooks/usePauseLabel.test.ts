import { describe, it, expect, afterEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import i18n from '@/lib/i18n';
import { usePauseLabel } from './usePauseLabel';

const now = new Date('2026-06-01T10:00:00Z');
const inAnHour = new Date(now.getTime() + 3600000).toISOString();

describe('usePauseLabel', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('says why notifications are paused', () => {
    const { result } = renderHook(() => usePauseLabel());
    expect(result.current({ until: inAnHour, bySchedule: true }, now)).toMatch(
      /^Paused by your schedule until /,
    );
    expect(result.current({ until: inAnHour, bySchedule: false }, now)).toMatch(/^Paused until /);
  });

  it('says it in Japanese', async () => {
    await i18n.changeLanguage('ja');
    const { result } = renderHook(() => usePauseLabel());
    expect(result.current({ until: inAnHour, bySchedule: true }, now)).toMatch(
      /^通知設定により .+ まで一時停止中$/,
    );
    expect(result.current({ until: inAnHour, bySchedule: false }, now)).toMatch(/^.+ まで一時停止中$/);
  });
});
