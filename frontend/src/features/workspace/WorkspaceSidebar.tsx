import { useState, type FormEvent } from 'react';
import { Plus, ServerCrash, RefreshCw, Upload } from 'lucide-react';
import type { Workspace } from '@/stores/workspace';
import { useInstanceStore } from '@/stores/instances';
import { useWsStatusStore } from '@/stores/wsStatus';
import { instanceManager } from '@/lib/instances';
import { Modal } from '@/shared/components/Modal/Modal';
import { useWorkspaceUnreadCounts } from '@/hooks/queries/useNotifications';

interface Props {
  workspaces: Workspace[];
  deletedWorkspaces?: Workspace[];
  currentWorkspaceId: string | undefined;
  onSelectWorkspace: (ws: Workspace) => void;
  onCreateWorkspace: (name: string, instanceUrl: string) => Promise<void>;
  onAddInstance: () => void;
  onImportFromSlack: (instanceUrl: string) => void;
}

export default function WorkspaceSidebar({
  workspaces,
  deletedWorkspaces = [],
  currentWorkspaceId,
  onSelectWorkspace,
  onCreateWorkspace,
  onAddInstance,
  onImportFromSlack,
}: Props) {
  const { instances } = useInstanceStore();
  const wsStatuses = useWsStatusStore((s) => s.statuses);
  const unreadByWorkspace = useWorkspaceUnreadCounts(workspaces);
  const [showNewWs, setShowNewWs] = useState(false);
  const [newWsName, setNewWsName] = useState('');
  const [newWsInstanceUrl, setNewWsInstanceUrl] = useState('');

  const handleCreate = async (e: FormEvent) => {
    e.preventDefault();
    if (!newWsName.trim() || !newWsInstanceUrl) return;
    await onCreateWorkspace(newWsName.trim(), newWsInstanceUrl);
    setNewWsName('');
    setShowNewWs(false);
  };

  const openCreateModal = (instanceUrl: string) => {
    setNewWsInstanceUrl(instanceUrl);
    setNewWsName('');
    setShowNewWs(true);
  };

  const groups = instances.map((inst) => ({
    instance: inst,
    workspaces: workspaces.filter((ws) => ws.instanceUrl === inst.url),
    deletedWorkspaces: deletedWorkspaces.filter((ws) => ws.instanceUrl === inst.url),
  }));

  const instanceLabel = (url: string) => {
    try {
      return new URL(url).hostname;
    } catch {
      return url;
    }
  };

  return (
    <>
      <div
        role="navigation"
        aria-label="Workspaces"
        className="w-16 bg-rail flex flex-col items-center py-3 gap-1 border-r border-surface overflow-y-auto"
      >
        {groups.map((group, groupIdx) => (
          <div key={group.instance.url} className="w-full flex flex-col items-center gap-1">
            {groupIdx > 0 && <div className="w-8 h-px bg-raised my-1" />}

            <div
              className="w-full flex flex-col items-center gap-1"
              title={instanceLabel(group.instance.url)}
            >
              {(() => {
                const status = wsStatuses[group.instance.url];
                const dotColor =
                  status === 'connected'
                    ? 'bg-green-500'
                    : status === 'connecting'
                      ? 'bg-yellow-400'
                      : 'bg-subtle';
                const statusLabel =
                  status === 'connected'
                    ? 'Connected'
                    : status === 'connecting'
                      ? 'Connecting…'
                      : status === 'disconnected'
                        ? 'Disconnected'
                        : 'Not connected';
                return (
                  <div className="flex items-center gap-1 py-0.5">
                    <div
                      className={`w-1.5 h-1.5 rounded-full ${dotColor}`}
                      title={`${instanceLabel(group.instance.url)}: ${statusLabel}`}
                    />
                    {(status === 'disconnected' || (!status && instances.length > 0)) && (
                      <button
                        onClick={() => instanceManager.get(group.instance.url).ws.connect()}
                        className="text-muted hover:text-fg-dim transition cursor-pointer"
                        title="Retry connection"
                      >
                        <RefreshCw className="w-2.5 h-2.5" />
                      </button>
                    )}
                  </div>
                );
              })()}
              {group.workspaces.map((ws) => {
                const unread = unreadByWorkspace[ws.id] ?? 0;
                const showBadge = unread > 0 && currentWorkspaceId !== ws.id;
                return (
                  <button
                    key={ws.id}
                    onClick={() => onSelectWorkspace(ws)}
                    className={`relative w-10 h-10 rounded-xl overflow-hidden flex items-center justify-center text-sm font-bold transition cursor-pointer ${
                      currentWorkspaceId === ws.id
                        ? 'bg-purple-600 text-white ring-2 ring-purple-400'
                        : 'bg-raised text-fg-dim hover:bg-elevated'
                    }`}
                    title={`${ws.name} · ${instanceLabel(group.instance.url)}${showBadge ? ` · ${unread} unread` : ''}`}
                  >
                    {/* An icon if the workspace has picked one; the initial is
                        the fallback, not the only option. A column of identical
                        letters is not a switcher. */}
                    {ws.icon_url ? (
                      <img src={ws.icon_url} alt="" className="w-full h-full object-cover" />
                    ) : (
                      ws.name.charAt(0).toUpperCase()
                    )}
                    {showBadge && (
                      <span
                        aria-label={`${unread} unread notifications`}
                        className="absolute -top-1 -right-1 min-w-4 h-4 px-1 bg-red-500 text-white text-[9px] font-bold rounded-full flex items-center justify-center leading-none"
                      >
                        {unread > 99 ? '99+' : unread}
                      </span>
                    )}
                  </button>
                );
              })}

              {group.deletedWorkspaces.map((ws) => (
                <button
                  key={ws.id}
                  onClick={() => onSelectWorkspace(ws)}
                  className="w-10 h-10 rounded-xl flex items-center justify-center text-sm font-bold transition cursor-pointer opacity-40 hover:opacity-70 bg-raised text-muted relative"
                  title={`${ws.name} (deleted) · ${instanceLabel(group.instance.url)}`}
                >
                  <span className="line-through">{ws.name.charAt(0).toUpperCase()}</span>
                </button>
              ))}

              <button
                onClick={() => openCreateModal(group.instance.url)}
                className="w-10 h-10 rounded-xl flex items-center justify-center bg-surface text-muted hover:bg-raised hover:text-fg transition cursor-pointer"
                title={`Create workspace on ${instanceLabel(group.instance.url)}`}
              >
                <Plus className="w-4 h-4" />
              </button>
            </div>
          </div>
        ))}

        {groups.length > 0 && <div className="w-8 h-px bg-raised my-1" />}

        <button
          onClick={onAddInstance}
          className="w-10 h-10 rounded-xl flex items-center justify-center bg-surface text-muted hover:bg-raised hover:text-fg transition cursor-pointer"
          title="Add instance"
        >
          <ServerCrash className="w-4 h-4" />
        </button>
      </div>

      {showNewWs && (
        <Modal title="Create Workspace" onClose={() => setShowNewWs(false)} dataQa="create-workspace-modal">
          <form onSubmit={handleCreate}>
            <h2 className="text-lg font-bold mb-1">Create Workspace</h2>
            <p className="text-xs text-muted mb-4">{instanceLabel(newWsInstanceUrl)}</p>

            {/* The importer can create the workspace itself, and this is where
                somebody looks for it — not inside the menu of a workspace they
                have not made yet. */}
            <button
              type="button"
              onClick={() => {
                setShowNewWs(false);
                onImportFromSlack(newWsInstanceUrl);
              }}
              data-qa="create-workspace-import"
              className="w-full mb-4 flex items-center gap-3 px-3 py-2.5 rounded-lg border border-line hover:border-line-strong hover:bg-raised/40 text-left transition cursor-pointer"
            >
              <Upload className="w-4 h-4 text-accent shrink-0" />
              <span>
                <span className="block text-sm text-fg-soft">Import from Slack instead</span>
                <span className="block text-xs text-muted">
                  Bring a workspace export in, with its history
                </span>
              </span>
            </button>

            <input
              type="text"
              value={newWsName}
              onChange={(e) => setNewWsName(e.target.value)}
              placeholder="Workspace name"
              className="w-full px-4 py-3 bg-raised/50 border border-line-strong rounded-lg text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-purple-500 mb-4"
              required
            />
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setShowNewWs(false)}
                className="px-4 py-2 text-muted hover:text-fg transition"
              >
                Cancel
              </button>
              <button
                type="submit"
                className="px-4 py-2 bg-purple-600 hover:bg-purple-500 text-white rounded-lg transition"
              >
                Create
              </button>
            </div>
          </form>
        </Modal>
      )}
    </>
  );
}
