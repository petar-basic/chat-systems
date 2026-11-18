import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  useAppHome,
  useAppHomeAction,
  useAppHomeOpened,
  type ActiveApp,
  type HomeAction,
} from '@/features/apps';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { QueryState } from '@/shared/components/QueryState/QueryState';
import { BlockView } from './blocks/BlockView';
import { BlockActionsContext, actionKey, refusalText, type BlockActions } from './blocks/blockActions';

interface Props {
  app: ActiveApp;
  channelId: string;
}

function useHomeActions(app: ActiveApp): BlockActions | null {
  const [failure, setFailure] = useState<BlockActions['failure']>(null);
  const { mutate, isPending, variables } = useAppHomeAction(app.app_id);
  const pending = isPending && variables ? actionKey(variables.blockId, variables.actionId) : null;
  return useMemo(
    () =>
      app.interactive
        ? {
            act: (blockId: string, actionId: string, value?: string) => {
              setFailure(null);
              mutate({ blockId, actionId, value } satisfies HomeAction, {
                onError: (error) => setFailure({ blockId, text: refusalText(error) }),
              });
            },
            pending,
            failure,
          }
        : null,
    [app.interactive, mutate, pending, failure],
  );
}

export function AppHomeTab({ app, channelId }: Props) {
  const { t } = useTranslation('messages');
  const home = useAppHome(app.app_id);
  const { mutate: tellOpened } = useAppHomeOpened(app.app_id);
  const actions = useHomeActions(app);

  useEffect(() => {
    tellOpened(channelId);
  }, [tellOpened, channelId]);

  const view = home.data?.home;

  return (
    <div className="flex-1 overflow-y-auto px-6 py-5" data-qa="app-home">
      <QueryState
        isLoading={home.isPending}
        isError={home.isError}
        onRetry={() => void home.refetch()}
        isEmpty={!view}
        empty={
          <div
            className="mx-auto flex max-w-md flex-col items-center gap-3 py-12 text-center"
            data-qa="app-home-empty"
          >
            <Avatar userId={app.bot_user_id} name={app.name} avatarUrl={app.icon_url} size="xl" />
            <h2 className="text-lg font-semibold text-fg">{app.name}</h2>
            {app.description && <p className="text-sm text-fg-dim">{app.description}</p>}
            <p className="text-xs text-muted">{t('appHome.empty', { name: app.name })}</p>
          </div>
        }
      >
        {view && (
          <BlockActionsContext.Provider value={actions}>
            <div className="flex max-w-2xl flex-col gap-3">
              {view.blocks.map((block, index) => (
                <BlockView key={index} block={block} />
              ))}
            </div>
          </BlockActionsContext.Provider>
        )}
      </QueryState>
    </div>
  );
}
