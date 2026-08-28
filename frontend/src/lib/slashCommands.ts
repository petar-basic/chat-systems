import { api } from './api';
import { instanceManager } from './instances';
import { ApiError } from './errors';
import { formatDateTime } from './datetime';

export interface CommandResult {
  response_type: 'ephemeral' | 'in_channel';
  text: string;
  at?: string;
}

export function commandResultText(result: Pick<CommandResult, 'text' | 'at'>): string {
  if (!result.at) return result.text;
  const when = formatDateTime(result.at);
  return when ? `${result.text} ${when}.` : result.text;
}

/** `/deploy prod` → `{ command: 'deploy', text: 'prod' }`. */
export function parseCommand(content: string): { command: string; text: string } | null {
  const trimmed = content.trim();
  if (!trimmed.startsWith('/')) return null;

  const match = /^\/([a-z0-9_-]{2,32})(?:\s+([\s\S]*))?$/i.exec(trimmed);
  if (!match) return null;
  return { command: match[1].toLowerCase(), text: match[2]?.trim() ?? '' };
}

/**
 * Returns the answer, or `null` when the server does not know the command — the
 * caller then sends what was typed as an ordinary message, so a typo is visible
 * rather than swallowed by an error dialog.
 */
export async function runCommand(
  channelId: string,
  content: string,
  instanceUrl?: string,
): Promise<CommandResult | null> {
  const parsed = parseCommand(content);
  if (!parsed) return null;

  const client = instanceUrl ? instanceManager.get(instanceUrl).api : api;
  try {
    return await client.post<CommandResult>(`/channels/${channelId}/commands`, parsed);
  } catch (e) {
    if (e instanceof ApiError && e.status === 404) return null;
    throw e;
  }
}
