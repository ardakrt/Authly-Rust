import { describe, expect, it } from 'vitest';
import type { AccountDto } from '@shared/schemas/account';
import {
  filterAndRankAccounts,
  formatTotpCode,
  hasTotpPeriodRolled,
  matchesSearchQuery,
} from '../../src/renderer/src/utils/accountHelpers';

function makeAccount(
  id: string,
  issuer: string,
  accountName: string,
  overrides: Partial<AccountDto> = {},
): AccountDto {
  return {
    id,
    issuer,
    accountName,
    algorithm: 'SHA1',
    digits: 6,
    period: 30,
    favorite: false,
    groupId: null,
    sortOrder: 0,
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}

describe('accountHelpers', () => {
  it('formats 6-digit and 8-digit TOTP codes in 3-digit groups', () => {
    expect(formatTotpCode('123456')).toBe('123 456');
    expect(formatTotpCode('12345678')).toBe('123 456 78');
  });

  it('detects when any account period rolls over between seconds', () => {
    expect(hasTotpPeriodRolled([{ period: 30 }], 29, 30)).toBe(true);
    expect(hasTotpPeriodRolled([{ period: 30 }], 30, 31)).toBe(false);
    expect(hasTotpPeriodRolled([{ period: 30 }, { period: 60 }], 59, 60)).toBe(true);
    expect(hasTotpPeriodRolled([{ period: 30 }], 30, 30)).toBe(false);
  });

  it('matches Turkish and English case variations without false positives', () => {
    expect(matchesSearchQuery('Instagram', 'instagram')).toBe(true);
    expect(matchesSearchQuery('Instagram', 'INSTAGRAM')).toBe(true);
    expect(matchesSearchQuery('İş Bankası', 'iş')).toBe(true);
    expect(matchesSearchQuery('İş Bankası', 'İŞ')).toBe(true);
    expect(matchesSearchQuery('Discord', 'iş')).toBe(false);
    expect(matchesSearchQuery('Ayarları aç', 'ayar')).toBe(true);
  });

  it('filters by issuer and accountName and ranks issuer prefix matches first', () => {
    const github = makeAccount('1', 'GitHub', 'discord-backup@example.com');
    const discord = makeAccount('2', 'Discord', 'arda@example.com');
    const myDiscord = makeAccount('3', 'MyDiscordAlt', 'work@example.com');

    expect(filterAndRankAccounts([github, discord, myDiscord], '')).toEqual([
      github,
      discord,
      myDiscord,
    ]);

    expect(filterAndRankAccounts([github, discord, myDiscord], 'Discord')).toEqual([
      discord,
      myDiscord,
      github,
    ]);

    expect(filterAndRankAccounts([github, discord, myDiscord], 'discord work')).toEqual([
      myDiscord,
    ]);
    expect(filterAndRankAccounts([github, discord, myDiscord], 'arda@')).toEqual([discord]);
  });
});
