import { useState, type FormEvent } from 'react';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { Button } from '@/shared/components/Button/Button';
import { RadioCards } from '@/shared/components/Form/RadioCards';
import { PanelHeader } from '@/shared/components/PanelHeader/PanelHeader';
import { getApiForInstance } from '@/shared/hooks/useCurrentApi';
import { useNavigate } from 'react-router';
import { Save, Trash2, AlertTriangle, RotateCcw } from 'lucide-react';
import { useRestoreWorkspace } from '../hooks/queries/useWorkspaces';
import { useQueryClient } from '@tanstack/react-query';
import { api } from '../lib/api';
import { logger } from '@/lib/logger';
import { toast } from '@/shared/components/Toast';
import { ErrorLabels, QUERY_KEYS } from '@/shared/constants';
import { toUserMessage } from '@/lib/errors';
import { Modal } from '@/shared/components/Modal/Modal';
import { TextInput } from '@/shared/components/Form/TextInput';
import { TextArea } from '@/shared/components/Form/TextArea';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { FilePicker } from '@/shared/components/Form/FilePicker';

interface Props {
  workspaceId: string;
  currentIconUrl?: string | null;
  instanceUrl?: string;
  currentName: string;
  currentDescription: string | null;
  currentLinkPreviews: boolean;
  deletedAt?: string | null;
  onClose: () => void;
}

type DeleteType = 'soft' | 'hard';

export default function SettingsPanel({
  workspaceId,
  currentIconUrl,
  instanceUrl,
  currentName,
  currentDescription,
  currentLinkPreviews,
  deletedAt,
  onClose,
}: Props) {
  const navigate = useNavigate();
  const [name, setName] = useState(currentName);
  const [iconUrl, setIconUrl] = useState(currentIconUrl ?? '');
  const [uploadingIcon, setUploadingIcon] = useState(false);
  const [description, setDescription] = useState(currentDescription || '');
  const [linkPreviews, setLinkPreviews] = useState(currentLinkPreviews);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [deleteModal, setDeleteModal] = useState(false);
  const [deleteType, setDeleteType] = useState<DeleteType>('soft');
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const queryClient = useQueryClient();
  const restoreMutation = useRestoreWorkspace();

  const getApi = () => {
    if (instanceUrl) return getApiForInstance(instanceUrl);
    return api;
  };

  const handleIconUpload = async ([file]: File[]) => {
    setUploadingIcon(true);
    setError(null);
    try {
      const formData = new FormData();
      formData.append('file', file);
      const uploaded = await getApi().upload<{ url: string }[]>(`/files/upload/${workspaceId}`, formData);
      const url = uploaded[0]?.url;
      if (url) setIconUrl(url);
      else setError('Failed to upload the icon');
    } catch (err) {
      setError(toUserMessage(err));
    } finally {
      setUploadingIcon(false);
    }
  };

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;

    setSaving(true);
    setError(null);
    setSaved(false);
    try {
      await getApi().typed((c) =>
        c.PATCH('/workspaces/{ws_id}', {
          params: { path: { ws_id: workspaceId } },
          body: {
            // Empty string, not null: null means "unchanged" to the API.
            icon_url: iconUrl.trim(),
            name: name.trim(),
            description: description.trim() || null,
            link_previews: linkPreviews,
          },
        }),
      );
      setSaved(true);
      queryClient.invalidateQueries({ queryKey: QUERY_KEYS.workspaces() });
      setTimeout(() => setSaved(false), 2000);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Failed to save settings';
      setError(msg);
    } finally {
      setSaving(false);
    }
  };

  const handleRestore = async () => {
    if (!instanceUrl) return;
    try {
      await restoreMutation.mutateAsync({ workspaceId, instanceUrl });
      onClose();
    } catch (err: unknown) {
      logger.error('SettingsPanel', 'handleRestore', err);
      toast.error(toUserMessage(err) || ErrorLabels.RestoreFailed);
    }
  };

  const handleDelete = async () => {
    setDeleting(true);
    setDeleteError(null);
    try {
      const hard = deleteType === 'hard';
      await getApi().typed((c) =>
        c.DELETE('/workspaces/{ws_id}', {
          params: { path: { ws_id: workspaceId }, query: hard ? { hard } : undefined },
        }),
      );
      setDeleteModal(false);
      onClose();
      navigate('/app');
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Failed to delete workspace';
      setDeleteError(msg);
    } finally {
      setDeleting(false);
    }
  };

  return (
    <>
      <div className="w-80 bg-surface/80 border-l border-line/50 flex flex-col h-full">
        <PanelHeader title="Workspace settings" onClose={onClose} />

        <form onSubmit={handleSave} className="flex-1 overflow-y-auto p-4 space-y-4">
          {/* A workspace picks its face the same way a person picks theirs.
              Without it the switcher is a column of identical initials, which
              on a phone has no tooltip to fall back on. */}
          <div className="flex items-center gap-3">
            <div className="w-12 h-12 rounded-xl overflow-hidden bg-raised flex items-center justify-center shrink-0">
              {iconUrl ? (
                <img src={iconUrl} alt="" className="w-full h-full object-cover" />
              ) : (
                <span className="text-lg font-bold text-fg-dim">{name.charAt(0).toUpperCase()}</span>
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
              {iconUrl && (
                <button
                  type="button"
                  onClick={() => setIconUrl('')}
                  className="text-xs text-muted hover:text-danger transition cursor-pointer"
                >
                  Remove
                </button>
              )}
            </div>
          </div>

          <TextInput label="Workspace name" value={name} onChange={(e) => setName(e.target.value)} required />

          <TextArea
            label="Description"
            optional
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="What is this workspace about?"
          />

          <Checkbox
            label="Link previews"
            description="Show a card with the title and image of pages linked in messages. The server fetches each public page to build it."
            checked={linkPreviews}
            onChange={setLinkPreviews}
            dataQa="workspace-link-previews"
          />

          {error && (
            <div className="text-sm text-danger bg-red-500/10 border border-red-500/30 rounded-lg px-3 py-2">
              {error}
            </div>
          )}

          {saved && (
            <div className="text-sm text-success bg-green-500/10 border border-green-500/30 rounded-lg px-3 py-2">
              Settings saved successfully.
            </div>
          )}

          <Button
            variant="primary"
            fullWidth
            type="submit"
            disabled={saving || !name.trim()}
            loading={saving}
            icon={<Save className="h-4 w-4" />}
          >
            Save changes
          </Button>

          <div className="border border-red-500/30 rounded-lg p-4 mt-6">
            <h3 className="text-sm font-semibold text-danger mb-2 flex items-center gap-1.5">
              <AlertTriangle className="w-4 h-4" />
              Danger zone
            </h3>
            {deletedAt ? (
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
        </form>
      </div>

      {deleteModal && (
        <Modal
          title={`Delete “${currentName}”`}
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
