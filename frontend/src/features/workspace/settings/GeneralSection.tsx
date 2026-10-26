import { useState } from 'react';
import { useNavigate } from 'react-router';
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
import { workflowBuildersOf, type WorkflowBuilders } from '@/features/workflows';
import { linkPreviewsEnabled } from '../linkPreviews';
import { workspaceInitials } from '../workspaceTile';

type Changes = components['schemas']['UpdateWorkspaceRequest'];
type DeleteType = 'soft' | 'hard';

interface Props {
  workspace: Workspace;
  onDeleted: () => void;
}

export function GeneralSection({ workspace, onDeleted }: Props) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const restoreMutation = useRestoreWorkspace();
  const client = getApiForInstance(workspace.instanceUrl);

  const [name, setName] = useState(workspace.name);
  const [description, setDescription] = useState(workspace.description ?? '');
  const [linkPreviews, setLinkPreviews] = useState(linkPreviewsEnabled(workspace.settings));
  const [workflowBuilders, setWorkflowBuilders] = useState(workflowBuildersOf(workspace.settings));
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
    onSuccess: () => queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() }),
  });

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
      const uploaded = await client.upload<{ url: string }[]>(`/files/upload/${workspace.id}`, formData);
      const url = uploaded[0]?.url;
      if (url) save.mutate({ icon_url: url });
      else toast.error('Failed to upload the icon');
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
      setDeleteError(toUserMessage(err, 'Failed to delete workspace'));
    } finally {
      setDeleting(false);
    }
  };

  return (
    <>
      <div className="space-y-4" data-qa="workspace-general">
        <div className="flex items-center gap-3">
          <div className="w-12 h-12 rounded-xl overflow-hidden bg-raised flex items-center justify-center shrink-0">
            {workspace.icon_url ? (
              <img src={workspace.icon_url} alt="" className="w-full h-full object-cover" />
            ) : (
              <span className="text-lg font-bold text-fg-dim">{workspaceInitials(workspace.name)}</span>
            )}
          </div>
          <div className="flex items-center gap-2">
            <FilePicker
              label="Workspace icon"
              accept="image/*"
              onFiles={handleIconUpload}
              disabled={uploadingIcon}
              dataQa="workspace-icon-upload"
              className="px-3 py-2 text-sm bg-raised hover:bg-elevated text-fg rounded-lg transition"
            >
              {uploadingIcon ? 'Uploading…' : 'Upload icon'}
            </FilePicker>
            {workspace.icon_url && (
              <button
                type="button"
                onClick={() => save.mutate({ icon_url: '' })}
                className="text-xs text-muted hover:text-danger transition cursor-pointer"
              >
                Remove
              </button>
            )}
          </div>
        </div>

        <TextInput
          label="Workspace name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={saveName}
          dataQa="workspace-name"
          required
        />

        <TextArea
          label="Description"
          optional
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          onBlur={saveDescription}
          placeholder="What is this workspace about?"
        />

        <Checkbox
          label="Link previews"
          description="Show a card with the title and image of pages linked in messages. The server fetches each public page to build it."
          checked={linkPreviews}
          onChange={(link_previews) => {
            setLinkPreviews(link_previews);
            save.mutate({ link_previews });
          }}
          dataQa="workspace-link-previews"
        />

        <Select
          label="Who can build workflows"
          value={workflowBuilders}
          onChange={(e) => {
            const workflow_builders = e.target.value as WorkflowBuilders;
            setWorkflowBuilders(workflow_builders);
            save.mutate({ workflow_builders });
          }}
          dataQa="workspace-workflow-builders"
        >
          <option value="members">Every member</option>
          <option value="admins">Admins only</option>
        </Select>

        <p aria-live="polite" className="min-h-5 text-xs" data-qa="workspace-save-status">
          {save.isPending ? (
            <span className="text-muted">Saving…</span>
          ) : save.isError ? (
            <span className="text-danger">{toUserMessage(save.error, 'Failed to save settings')}</span>
          ) : save.isSuccess ? (
            <span className="text-success">Saved</span>
          ) : null}
        </p>

        <div className="border border-red-500/30 rounded-lg p-4">
          <h3 className="text-sm font-semibold text-danger mb-2 flex items-center gap-1.5">
            <AlertTriangle className="w-4 h-4" />
            Danger zone
          </h3>
          {workspace.deleted_at ? (
            <>
              <p className="text-xs text-muted mb-3">
                This workspace was soft-deleted. You can restore it to make it visible to members again.
              </p>
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
                    Restore workspace
                  </>
                )}
              </button>
            </>
          ) : (
            <>
              <p className="text-xs text-muted mb-3">
                Deleting a workspace is irreversible (hard) or hides it from members (soft).
              </p>
              <button
                type="button"
                onClick={() => setDeleteModal(true)}
                className="w-full flex items-center justify-center gap-2 px-3 py-2 bg-red-600/20 hover:bg-red-600/40 border border-red-500/40 text-danger text-sm rounded-lg transition cursor-pointer"
              >
                <Trash2 className="w-4 h-4" />
                Delete workspace
              </button>
            </>
          )}
        </div>
      </div>

      {deleteModal && (
        <Modal
          title={`Delete “${workspace.name}”`}
          description="Choose how to delete this workspace"
          onClose={() => {
            setDeleteModal(false);
            setDeleteError(null);
          }}
          size="sm"
          dataQa="delete-workspace-modal"
          dismissLabel="Cancel"
          actions={
            <Button
              variant="danger"
              onClick={handleDelete}
              loading={deleting}
              icon={<Trash2 className="h-4 w-4" />}
            >
              {deleteType === 'hard' ? 'Delete permanently' : 'Soft delete'}
            </Button>
          }
        >
          <RadioCards
            label="How to delete"
            labelHidden
            value={deleteType}
            onChange={setDeleteType}
            options={[
              {
                value: 'soft',
                label: 'Soft delete',
                description:
                  'Hides the workspace from members. Only instance admins and workspace admins can see and restore it.',
              },
              {
                value: 'hard',
                label: 'Delete permanently',
                description:
                  'Permanently deletes everything: channels, messages, members. This cannot be undone.',
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
