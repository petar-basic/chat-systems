import { Fragment, useEffect, useState } from 'react';
import { useNavigate } from 'react-router';
import { useTranslation } from 'react-i18next';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Trash2, AlertTriangle, RotateCcw } from 'lucide-react';
import type { components } from '@/api/schema';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { useRestoreWorkspace } from '@/features/workspace/hooks/useWorkspaces';
import { logger } from '@/lib/logger';
import { toast } from '@/shared/components/Toast';
import { ErrorLabels, QUERY_KEYS } from '@/shared/constants';
import { toUserMessage } from '@/lib/errors';
import { Modal } from '@/shared/components/Modal/Modal';
import { TextInput } from '@/shared/components/Form/TextInput';
import { TextArea } from '@/shared/components/Form/TextArea';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { FilePicker } from '@/shared/components/Form/FilePicker';
import { Select } from '@/shared/components/Form/Select';
import type { Workspace } from '@/stores/workspace';
import { ruleOf, type WhoMay, type WorkspaceRule } from '@/lib/workspaceRules';
import { GIF_RATINGS, gifRatingOf, gifsEnabled, type GifRating } from '@/features/gifs';
import { linkPreviewsEnabled } from '../linkPreviews';
import { WorkspaceIcon } from '../components/WorkspaceIcon';

type Changes = components['schemas']['UpdateWorkspaceRequest'];
type DeleteType = 'soft' | 'hard';

const PERMISSIONS: WorkspaceRule[] = [
  'workflow_builders',
  'broadcast_mentions',
  'public_channel_creators',
  'private_channel_creators',
  'inviters',
  'custom_responders',
];

interface Props {
  workspace: Workspace;
  onDeleted: () => void;
}

export function GeneralSection({ workspace, onDeleted }: Props) {
  const { t } = useTranslation(['workspace', 'common']);
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const restoreMutation = useRestoreWorkspace();
  const client = getApiForInstance(workspace.instanceUrl);

  const [name, setName] = useState(workspace.name);
  const [description, setDescription] = useState(workspace.description ?? '');
  const [linkPreviews, setLinkPreviews] = useState(linkPreviewsEnabled(workspace.settings));
  const [gifs, setGifs] = useState(gifsEnabled(workspace.settings));
  const [gifRating, setGifRating] = useState(gifRatingOf(workspace.settings));
  const [rules, setRules] = useState(() =>
    Object.fromEntries(PERMISSIONS.map((rule) => [rule, ruleOf(workspace.settings, rule)])),
  );
  const [uploadingIcon, setUploadingIcon] = useState(false);
  const [deleteModal, setDeleteModal] = useState(false);
  const [deleteType, setDeleteType] = useState<DeleteType>('soft');
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const save = useMutation({
    mutationFn: (changes: Changes) =>
      client.typed((c) =>
        c.PATCH('/workspaces/{ws_id}', { params: { path: { ws_id: workspace.id } }, body: changes }),
      ),
    onSuccess: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.gifSettings(workspace.id) }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.slashCommands(workspace.id) }),
      ]),
  });

  const { isSuccess: saved, reset: resetSave } = save;
  useEffect(() => {
    if (!saved) return undefined;
    const timer = setTimeout(resetSave, 2000);
    return () => clearTimeout(timer);
  }, [saved, resetSave]);

  const statusOf = (field: keyof Changes) =>
    save.variables && field in save.variables ? (
      <p aria-live="polite" className="-mt-2 text-xs" data-qa="workspace-save-status">
        {save.isPending ? (
          <span className="text-muted">{t('settings.general.saving')}</span>
        ) : save.isError ? (
          <span className="text-danger">{toUserMessage(save.error, t('settings.general.saveFailed'))}</span>
        ) : save.isSuccess ? (
          <span className="text-success">{t('settings.general.saved')}</span>
        ) : null}
      </p>
    ) : null;

  const saveName = () => {
    const trimmed = name.trim();
    if (!trimmed) {
      setName(workspace.name);
      return;
    }
    if (trimmed !== workspace.name) save.mutate({ name: trimmed });
  };

  const saveDescription = () => {
    const trimmed = description.trim();
    if (trimmed !== (workspace.description ?? '')) save.mutate({ description: trimmed });
  };

  const handleIconUpload = async ([file]: File[]) => {
    setUploadingIcon(true);
    try {
      const formData = new FormData();
      formData.append('file', file);
      await client.upload<components['schemas']['Workspace']>(`/workspaces/${workspace.id}/icon`, formData);
      await queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
    } catch (err) {
      toast.error(toUserMessage(err));
    } finally {
      setUploadingIcon(false);
    }
  };

  const handleRestore = async () => {
    try {
      await restoreMutation.mutateAsync({ workspaceId: workspace.id, instanceUrl: workspace.instanceUrl });
    } catch (err: unknown) {
      logger.error('GeneralSection', 'handleRestore', err);
      toast.error(toUserMessage(err) || ErrorLabels.RestoreFailed);
    }
  };

  const handleDelete = async () => {
    setDeleting(true);
    setDeleteError(null);
    try {
      const hard = deleteType === 'hard';
      await client.typed((c) =>
        c.DELETE('/workspaces/{ws_id}', {
          params: { path: { ws_id: workspace.id }, query: hard ? { hard } : undefined },
        }),
      );
      setDeleteModal(false);
      onDeleted();
      navigate('/app');
    } catch (err: unknown) {
      setDeleteError(toUserMessage(err, t('settings.general.deleteFailed')));
    } finally {
      setDeleting(false);
    }
  };

  return (
    <>
      <div className="space-y-4" data-qa="workspace-general">
        <div className="flex items-center gap-3">
          <WorkspaceIcon
            workspace={workspace}
            className="w-12 h-12 rounded-xl shrink-0"
            initialsClassName="bg-raised text-lg font-bold text-fg-dim"
          />
          <div className="flex items-center gap-2">
            <FilePicker
              label={t('settings.general.icon')}
              accept="image/*"
              onFiles={handleIconUpload}
              disabled={uploadingIcon}
              dataQa="workspace-icon-upload"
              className="px-3 py-2 text-sm bg-raised hover:bg-elevated text-fg rounded-lg transition"
            >
              {uploadingIcon ? t('settings.general.uploading') : t('settings.general.uploadIcon')}
            </FilePicker>
            {workspace.icon_url && (
              <button
                type="button"
                onClick={() => save.mutate({ icon_url: '' })}
                className="text-xs text-muted hover:text-danger transition cursor-pointer"
              >
                {t('settings.general.removeIcon')}
              </button>
            )}
          </div>
        </div>

        <TextInput
          label={t('settings.general.name')}
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={saveName}
          dataQa="workspace-name"
          required
        />
        {statusOf('name')}

        <TextArea
          label={t('settings.general.description')}
          optional
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          onBlur={saveDescription}
          placeholder={t('settings.general.descriptionPlaceholder')}
        />
        {statusOf('description')}

        <Checkbox
          label={t('settings.general.linkPreviews')}
          description={t('settings.general.linkPreviewsHint')}
          checked={linkPreviews}
          onChange={(link_previews) => {
            setLinkPreviews(link_previews);
            save.mutate({ link_previews });
          }}
          dataQa="workspace-link-previews"
        />
        {statusOf('link_previews')}

        <Checkbox
          label={t('settings.general.gifs')}
          description={t('settings.general.gifsHint')}
          checked={gifs}
          onChange={(on) => {
            setGifs(on);
            save.mutate({ gifs: on });
          }}
          dataQa="workspace-gifs"
        />
        {statusOf('gifs')}

        {gifs && (
          <>
            <Select
              label={t('settings.general.gifRating')}
              hint={t('settings.general.gifRatingHint')}
              value={gifRating}
              onChange={(e) => {
                const rating = e.target.value as GifRating;
                setGifRating(rating);
                save.mutate({ gif_rating: rating });
              }}
              dataQa="workspace-gif-rating"
            >
              {GIF_RATINGS.map((rating) => (
                <option key={rating} value={rating}>
                  {t(`settings.general.gifRatings.${rating}`)}
                </option>
              ))}
            </Select>
            {statusOf('gif_rating')}
          </>
        )}

        <fieldset className="space-y-3" data-qa="workspace-permissions">
          <legend className="mb-2 text-sm font-semibold text-fg">{t('settings.general.permissions')}</legend>
          {PERMISSIONS.map((rule) => (
            <Fragment key={rule}>
              <Select
                label={t(`settings.general.rules.${rule}`)}
                value={rules[rule]}
                onChange={(e) => {
                  const who = e.target.value as WhoMay;
                  setRules((current) => ({ ...current, [rule]: who }));
                  save.mutate({ [rule]: who });
                }}
                dataQa={`workspace-${rule.replaceAll('_', '-')}`}
              >
                <option value="members">{t('settings.general.everyMember')}</option>
                <option value="admins">{t('settings.general.adminsOnly')}</option>
              </Select>
              {statusOf(rule)}
            </Fragment>
          ))}
        </fieldset>

        <div className="border border-red-500/30 rounded-lg p-4">
          <h3 className="text-sm font-semibold text-danger mb-2 flex items-center gap-1.5">
            <AlertTriangle className="w-4 h-4" />
            {t('settings.general.dangerZone')}
          </h3>
          {workspace.deleted_at ? (
            <>
              <p className="text-xs text-muted mb-3">{t('settings.general.softDeleted')}</p>
              <button
                type="button"
                onClick={handleRestore}
                disabled={restoreMutation.isPending}
                className="w-full flex items-center justify-center gap-2 px-3 py-2 bg-green-600/20 hover:bg-green-600/40 border border-green-500/40 text-success text-sm rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
              >
                {restoreMutation.isPending ? (
                  <Spinner size="sm" className="text-success" />
                ) : (
                  <>
                    <RotateCcw className="w-4 h-4" />
                    {t('settings.general.restore')}
                  </>
                )}
              </button>
            </>
          ) : (
            <>
              <p className="text-xs text-muted mb-3">{t('settings.general.deleteExplain')}</p>
              <button
                type="button"
                onClick={() => setDeleteModal(true)}
                className="w-full flex items-center justify-center gap-2 px-3 py-2 bg-red-600/20 hover:bg-red-600/40 border border-red-500/40 text-danger text-sm rounded-lg transition cursor-pointer"
              >
                <Trash2 className="w-4 h-4" />
                {t('settings.general.delete')}
              </button>
            </>
          )}
        </div>
      </div>

      {deleteModal && (
        <Modal
          title={t('settings.general.deleteTitle', { name: workspace.name })}
          description={t('settings.general.deleteDescription')}
          onClose={() => {
            setDeleteModal(false);
            setDeleteError(null);
          }}
          size="sm"
          dataQa="delete-workspace-modal"
          dismissLabel={t('cancel', { ns: 'common' })}
          actions={
            <Button
              variant="danger"
              onClick={handleDelete}
              loading={deleting}
              icon={<Trash2 className="h-4 w-4" />}
            >
              {deleteType === 'hard' ? t('settings.general.hard') : t('settings.general.soft')}
            </Button>
          }
        >
          <RadioCards
            label={t('settings.general.howToDelete')}
            labelHidden
            value={deleteType}
            onChange={setDeleteType}
            options={[
              {
                value: 'soft',
                label: t('settings.general.soft'),
                description: t('settings.general.softHint'),
              },
              {
                value: 'hard',
                label: t('settings.general.hard'),
                description: t('settings.general.hardHint'),
              },
            ]}
            columns={1}
          />

          {deleteError && (
            <p role="alert" className="mt-3 text-sm text-danger">
              {deleteError}
            </p>
          )}
        </Modal>
      )}
    </>
  );
}
