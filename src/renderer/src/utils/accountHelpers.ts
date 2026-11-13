import type { AccountDto } from '@shared/schemas/account';

/**
 * Formats a numeric TOTP code into 3-digit groups for readability.
 */
export function formatTotpCode(code: string): string {
  return code.replace(/(.{3})/g, '$1 ').trim();
}

/**
 * Returns true when any account's TOTP time window rolled over between two Unix seconds.
 */
export function hasTotpPeriodRolled(
  accounts: readonly Pick<AccountDto, 'period'>[],
  prevSec: number,
  nextSec: number,
): boolean {
  if (prevSec === nextSec) return false;
  return accounts.some((account) => {
    const period = account.period || 30;
    return Math.floor(prevSec / period) !== Math.floor(nextSec / period);
  });
}

function includesCaseInsensitive(text: string, token: string): boolean {
  return (
    text.toLocaleLowerCase('tr').includes(token.toLocaleLowerCase('tr')) ||
    text.toLowerCase().includes(token.toLowerCase())
  );
}

function startsWithCaseInsensitive(text: string, prefix: string): boolean {
  return (
    text.toLocaleLowerCase('tr').startsWith(prefix.toLocaleLowerCase('tr')) ||
    text.toLowerCase().startsWith(prefix.toLowerCase())
  );
}

/**
 * Matches multi-token search queries case-insensitively across both Turkish and default locales.
 */
export function matchesSearchQuery(text: string, query: string): boolean {
  const trimmed = query.trim();
  if (!trimmed) return true;
  const tokens = trimmed.split(/\s+/).filter(Boolean);
  return tokens.every((token) => includesCaseInsensitive(text, token));
}

function scoreAccountMatch(account: AccountDto, normalizedQuery: string): number {
  if (startsWithCaseInsensitive(account.issuer, normalizedQuery)) return 0;
  if (includesCaseInsensitive(account.issuer, normalizedQuery)) return 1;
  if (startsWithCaseInsensitive(account.accountName, normalizedQuery)) return 2;
  return 3;
}

/**
 * Filters accounts by issuer/accountName tokens and ranks issuer/prefix matches first.
 */
export function filterAndRankAccounts(
  accounts: readonly AccountDto[],
  query: string,
): AccountDto[] {
  const trimmed = query.trim();
  if (!trimmed) return [...accounts];

  const tokens = trimmed.split(/\s+/).filter(Boolean);
  return accounts
    .map((account, index) => ({ account, index }))
    .filter(({ account }) =>
      tokens.every(
        (token) =>
          includesCaseInsensitive(account.issuer, token) ||
          includesCaseInsensitive(account.accountName, token),
      ),
    )
    .sort((a, b) => {
      const rankDiff =
        scoreAccountMatch(a.account, trimmed) - scoreAccountMatch(b.account, trimmed);
      return rankDiff !== 0 ? rankDiff : a.index - b.index;
    })
    .map(({ account }) => account);
}
