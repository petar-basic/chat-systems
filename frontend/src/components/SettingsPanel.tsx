import { useState, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { X, Save, Trash2, AlertTriangle, RotateCcw } from 'lucide-react';
import { useRestoreWorkspace } from '../hooks/queries/useWorkspaces';
import { useQueryClient } from '@tanstack/react-query';
import { instanceManager } from '../lib/instances';
import { api } from '../lib/api';
import { logger } from '@/lib/logger';
import { toast } from '@/shared/components/Toast';
import { ErrorLabels, QUERY_KEYS } from '@/shared/constants';
import { toUserMessage } from '@/lib/errors';
import { Modal } from '@/shared/components/Modal/Modal';

interface Props {
  workspaceId: string;
  instanceUrl?: string;
  currentName: string;
  currentDescription: string | null;
  deletedAt?: string | null;
  onClose: () => void;
}

type DeleteType = 'soft' | 'hard';

export default function SettingsPanel({
  workspaceId,
  instanceUrl,
  currentName,
  currentDescription,
  deletedAt,
  onClose,
}: Props) {
  const navigate = useNavigate();
  const [name, setName] = useState(currentName);
  const [description, setDescription] = useState(currentDescription || '');
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
    if (instanceUrl) return instanceManager.get(instanceUrl).api;
    return api;
  };

  const handleSave = async (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;

    setSaving(true);
    setError(null);
    setSaved(false);
    try {
      await getApi().patch(`/workspaces/${workspaceId}`, {
        name: name.trim(),
        description: description.trim() || null,
      });
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
      await getApi().delete(`/workspaces/${workspaceId}${hard ? '?hard=true' : ''}`);
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
      <div className="w-80 bg-slate-800/80 border-l border-slate-700/50 flex flex-col h-full">
        <div className="h-14 px-4 flex items-center justify-between border-b border-slate-700/50 shrink-0">
          <h2 className="font-semibold text-white">Workspace Settings</h2>
          <button onClick={onClose} className="text-slate-400 hover:text-white transition cursor-pointer">
            <X className="w-5 h-5" />
          </button>
        </div>

        <form onSubmit={handleSave} className="flex-1 overflow-y-auto p-4 space-y-4">
          <div>
            <label htmlFor="ws-name" className="block text-sm font-medium text-slate-300 mb-1.5">
              Workspace Name
            </label>
            <input
              id="ws-name"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full px-3 py-2.5 bg-slate-700/50 border border-slate-600 rounded-lg text-white text-sm placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500"
              required
            />
          </div>

          <div>
            <label htmlFor="ws-description" className="block text-sm font-medium text-slate-300 mb-1.5">
              Description
            </label>
            <textarea
              id="ws-description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={3}
              className="w-full px-3 py-2.5 bg-slate-700/50 border border-slate-600 rounded-lg text-white text-sm placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500 resize-none"
              placeholder="What is this workspace about?"
            />
          </div>

          {error && (
            <div className="text-sm text-red-400 bg-red-500/10 border border-red-500/30 rounded-lg px-3 py-2">
              {error}
            </div>
          )}

          {saved && (
            <div className="text-sm text-green-400 bg-green-500/10 border border-green-500/30 rounded-lg px-3 py-2">
              Settings saved successfully.
            </div>
          )}

          <button
            type="submit"
            disabled={saving || !name.trim()}
            className="w-full flex items-center justify-center gap-2 px-3 py-2.5 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white text-sm font-medium rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
          >
            {saving ? (
              <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
            ) : (
              <>
                <Save className="w-4 h-4" />
                Save Changes
              </>
            )}
          </button>

          <div className="border border-red-500/30 rounded-lg p-4 mt-6">
            <h3 className="text-sm font-semibold text-red-400 mb-2 flex items-center gap-1.5">
              <AlertTriangle className="w-4 h-4" />
              Danger Zone
            </h3>
            {deletedAt ? (
              <>
                <p className="text-xs text-slate-400 mb-3">
                  This workspace was soft-deleted. You can restore it to make it visible to members again.
                </p>
                <button
                  type="button"
                  onClick={handleRestore}
                  disabled={restoreMutation.isPending}
                  className="w-full flex items-center justify-center gap-2 px-3 py-2 bg-green-600/20 hover:bg-green-600/40 border border-green-500/40 text-green-400 text-sm rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
                >
                  {restoreMutation.isPending ? (
                    <div className="w-4 h-4 border-2 border-green-400/30 border-t-green-400 rounded-full animate-spin" />
                  ) : (
                    <>
                      <RotateCcw className="w-4 h-4" />
                      Restore Workspace
                    </>
                  )}
                </button>
              </>
            ) : (
              <>
                <p className="text-xs text-slate-400 mb-3">
                  Deleting a workspace is irreversible (hard) or hides it from members (soft).
                </p>
                <button
                  type="button"
                  onClick={() => setDeleteModal(true)}
                  className="w-full flex items-center justify-center gap-2 px-3 py-2 bg-red-600/20 hover:bg-red-600/40 border border-red-500/40 text-red-400 text-sm rounded-lg transition cursor-pointer"
                >
                  <Trash2 className="w-4 h-4" />
                  Delete Workspace
                </button>
              </>
            )}
          </div>
        </form>
      </div>

      {deleteModal && (
        <Modal
          title={`Delete ${currentName}`}
          onClose={() => {
            setDeleteModal(false);
            setDeleteError(null);
          }}
          dataQa="delete-workspace-modal"
          className="bg-slate-800 border border-slate-700 rounded-2xl p-6 w-full max-w-md shadow-2xl"
        >
          <div className="flex items-center gap-3 mb-4">
            <div className="w-10 h-10 bg-red-500/20 rounded-full flex items-center justify-center shrink-0">
              <Trash2 className="w-5 h-5 text-red-400" />
            </div>
            <div>
              <h3 className="text-white font-semibold">Delete "{currentName}"</h3>
              <p className="text-slate-400 text-sm">Choose how to delete this workspace</p>
            </div>
          </div>

          <div className="space-y-3 mb-5">
            <label
              className="flex items-start gap-3 p-3 rounded-lg border cursor-pointer transition"
              style={{
                borderColor: deleteType === 'soft' ? 'rgb(168 85 247 / 0.6)' : 'rgb(71 85 105 / 0.5)',
                background: deleteType === 'soft' ? 'rgb(168 85 247 / 0.1)' : 'transparent',
              }}
            >
              <input
                type="radio"
                name="deleteType"
                value="soft"
                checked={deleteType === 'soft'}
                onChange={() => setDeleteType('soft')}
                className="mt-0.5"
              />
              <div>
                <div className="text-white text-sm font-medium">Soft Delete</div>
                <div className="text-slate-400 text-xs mt-0.5">
                  Hides the workspace from members. Only instance admins and workspace admins can see and
                  restore it.
                </div>
              </div>
            </label>

            <label
              className="flex items-start gap-3 p-3 rounded-lg border cursor-pointer transition"
              style={{
                borderColor: deleteType === 'hard' ? 'rgb(239 68 68 / 0.6)' : 'rgb(71 85 105 / 0.5)',
                background: deleteType === 'hard' ? 'rgb(239 68 68 / 0.1)' : 'transparent',
              }}
            >
              <input
                type="radio"
                name="deleteType"
                value="hard"
                checked={deleteType === 'hard'}
                onChange={() => setDeleteType('hard')}
                className="mt-0.5"
              />
              <div>
                <div className="text-red-400 text-sm font-medium">Hard Delete</div>
                <div className="text-slate-400 text-xs mt-0.5">
                  Permanently deletes everything: channels, messages, members. This cannot be undone.
                </div>
              </div>
            </label>
          </div>

          {deleteError && (
            <div className="text-sm text-red-400 bg-red-500/10 border border-red-500/30 rounded-lg px-3 py-2 mb-4">
              {deleteError}
            </div>
          )}

          <div className="flex gap-3">
            <button
              onClick={() => {
                setDeleteModal(false);
                setDeleteError(null);
              }}
              disabled={deleting}
              className="flex-1 px-4 py-2.5 bg-slate-700 hover:bg-slate-600 text-white text-sm rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
            >
              Cancel
            </button>
            <button
              onClick={handleDelete}
              disabled={deleting}
              className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 bg-red-600 hover:bg-red-500 disabled:bg-red-600/50 text-white text-sm rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
            >
              {deleting ? (
                <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
              ) : (
                <>
                  <Trash2 className="w-4 h-4" />
                  {deleteType === 'hard' ? 'Permanently Delete' : 'Soft Delete'}
                </>
              )}
            </button>
          </div>
        </Modal>
      )}
    </>
  );
}
