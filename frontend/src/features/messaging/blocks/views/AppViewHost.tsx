import { useMemo, useState, type FormEvent } from 'react';
import { useMutation } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { ArrowLeft } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Button } from '@/shared/components/Button/Button';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { useCurrentApi } from '@/shared/hooks/useCurrentApi';
import { useAppViewsStore, type AppView, type ViewValues } from '@/stores/appViews';
import { BlockView } from '../BlockView';
import { BlockActionsContext, actionKey, refusalText, type BlockActions } from '../blockActions';
import { ViewInput } from './ViewInput';
import { currentValues, valueOf, type InputBlock } from './viewValues';

const NO_VIEWS: AppView[] = [];

export function AppViewHost({ workspaceId }: { workspaceId: string }) {
  const stack = useAppViewsStore((s) => s.stacks[workspaceId] ?? NO_VIEWS);
  const top = stack.at(-1);
  if (!top) return null;
  return <AppViewModal key={top.root_view_id} workspaceId={workspaceId} stack={stack} view={top} />;
}

interface Refusal {
  viewId: string;
  hash: string;
  errors: Record<string, string>;
}

interface ViewAction {
  viewId: string;
  blockId: string;
  actionId: string;
  value?: string;
  values: ViewValues;
}

function AppViewModal({
  workspaceId,
  stack,
  view,
}: {
  workspaceId: string;
  stack: AppView[];
  view: AppView;
}) {
  const { t } = useTranslation('messages');
  const { t: tc } = useTranslation('common');
  const apiClient = useCurrentApi();
  const stored = useAppViewsStore((s) => s.values[view.id]);
  const setValue = useAppViewsStore((s) => s.setValue);
  const drop = useAppViewsStore((s) => s.drop);
  const [refusal, setRefusal] = useState<Refusal | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [actionFailure, setActionFailure] = useState<BlockActions['failure']>(null);
  const errors = refusal && refusal.viewId === view.id && refusal.hash === view.hash ? refusal.errors : {};
  const pushed = stack.length > 1;

  const submit = useMutation({
    retry: 0,
    mutationFn: ({ viewId, values }: { viewId: string; values: ViewValues }) =>
      apiClient.typed((c) =>
        c.POST('/views/{view_id}/submit', { params: { path: { view_id: viewId } }, body: { values } }),
      ),
    onMutate: () => setFailure(null),
    onSuccess: (answer, { viewId }) => {
      if (answer.outcome === 'errors') setRefusal({ viewId, hash: view.hash, errors: answer.errors });
      if (answer.outcome === 'closed') drop(workspaceId, [viewId]);
      if (answer.outcome === 'cleared')
        drop(
          workspaceId,
          stack.map((each) => each.id),
        );
    },
    onError: (error) => setFailure(refusalText(error)),
  });

  const close = useMutation({
    retry: 0,
    mutationFn: ({ viewId, all }: { viewId: string; all: boolean }) =>
      apiClient.typed((c) =>
        c.POST('/views/{view_id}/close', { params: { path: { view_id: viewId } }, body: { all } }),
      ),
  });

  const {
    mutate: act,
    isPending: acting,
    variables: acted,
  } = useMutation({
    retry: 0,
    mutationFn: ({ viewId, blockId, actionId, value, values }: ViewAction) =>
      apiClient.typed((c) =>
        c.POST('/views/{view_id}/actions', {
          params: { path: { view_id: viewId } },
          body: { block_id: blockId, action_id: actionId, value, values },
        }),
      ),
    onMutate: () => setActionFailure(null),
    onError: (error, { blockId }) => setActionFailure({ blockId, text: refusalText(error) }),
  });

  const actions = useMemo<BlockActions>(
    () => ({
      act: (blockId, actionId, value) =>
        act({ viewId: view.id, blockId, actionId, value, values: currentValues(view, stored) }),
      pending: acting && acted ? actionKey(acted.blockId, acted.actionId) : null,
      failure: actionFailure,
    }),
    [act, acting, acted, actionFailure, view, stored],
  );

  const back = () => {
    drop(workspaceId, [view.id]);
    close.mutate({ viewId: view.id, all: false });
  };

  const closeAll = () => {
    drop(
      workspaceId,
      stack.map((each) => each.id),
    );
    close.mutate({ viewId: view.id, all: true });
  };

  const change = (block: InputBlock, next: string[]) => {
    const actionId = 'action_id' in block.element ? block.element.action_id : null;
    if (!block.block_id || !actionId) return;
    setValue(view.id, block.block_id, actionId, next);
    const blockId = block.block_id;
    if (refusal?.errors[blockId]) {
      setRefusal({
        ...refusal,
        errors: Object.fromEntries(Object.entries(refusal.errors).filter(([id]) => id !== blockId)),
      });
    }
  };

  const onSubmit = view.submit
    ? (e: FormEvent<HTMLFormElement>) => {
        e.preventDefault();
        submit.mutate({ viewId: view.id, values: currentValues(view, stored) });
      }
    : undefined;

  const closeLabel = view.close?.text ?? tc('cancel');

  return (
    <Modal
      title={view.title.text}
      onClose={closeAll}
      onSubmit={onSubmit}
      dataQa="app-view"
      headerActions={
        pushed && (
          <IconButton
            label={t('blocks.view.back')}
            icon={<ArrowLeft className="h-4 w-4" />}
            onClick={back}
            data-qa="app-view-back"
          />
        )
      }
      dismissLabel={pushed ? undefined : closeLabel}
      footerStart={
        failure && (
          <p role="alert" className="text-sm text-danger" data-qa="app-view-failure">
            {failure}
          </p>
        )
      }
      actions={
        <>
          {pushed && (
            <Button variant="ghost" onClick={back} data-qa="app-view-close">
              {closeLabel}
            </Button>
          )}
          {view.submit && (
            <Button type="submit" variant="primary" loading={submit.isPending} data-qa="app-view-submit">
              {view.submit.text}
            </Button>
          )}
        </>
      }
    >
      <BlockActionsContext.Provider value={actions}>
        <div key={view.id} className="flex flex-col gap-4 py-3" data-qa="app-view-blocks" data-view={view.id}>
          {view.blocks.map((block, index) =>
            block.type === 'input' ? (
              <ViewInput
                key={block.block_id ?? index}
                block={block}
                value={valueOf(block, stored)}
                error={errors[block.block_id ?? ''] ?? null}
                onChange={(next) => change(block, next)}
              />
            ) : (
              <BlockView key={block.block_id ?? index} block={block} />
            ),
          )}
        </div>
      </BlockActionsContext.Provider>
    </Modal>
  );
}
