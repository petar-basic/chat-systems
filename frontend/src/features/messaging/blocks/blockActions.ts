import { createContext, useContext } from 'react';
import i18n from '@/lib/i18n';
import { ApiError, toUserMessage } from '@/lib/errors';

export interface BlockActions {
  act: (blockId: string, actionId: string, value?: string) => void;
  pending: string | null;
  failure: { blockId: string; text: string } | null;
}

export const BlockActionsContext = createContext<BlockActions | null>(null);

export const useBlockActions = () => useContext(BlockActionsContext);

export const actionKey = (blockId: string, actionId: string) => `${blockId}/${actionId}`;

export function refusalText(error: unknown): string {
  return error instanceof ApiError && error.status === 503
    ? i18n.t('messages:blocks.noResponse')
    : toUserMessage(error, i18n.t('messages:blocks.failed'));
}
