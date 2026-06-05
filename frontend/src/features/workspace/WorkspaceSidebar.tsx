import { useState, type FormEvent } from 'react';
import { Plus, ServerCrash, RefreshCw } from 'lucide-react';
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
}

export default function WorkspaceSidebar({
  workspaces,
  deletedWorkspaces = [],
  currentWorkspaceId,
  onSelectWorkspace,
  onCreateWorkspace,
  onAddInstance,
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
        className="w-16 bg-slate-950 flex flex-col items-center py-3 gap-1 border-r border-slate-800 overflow-y-auto"
      >
        {groups.map((group, groupIdx) => (
          <div key={group.instance.url} className="w-full flex flex-col items-center gap-1">
            {groupIdx > 0 && <div className="w-8 h-px bg-slate-700 my-1" />}

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
                      : 'bg-slate-500';
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
                        className="text-slate-400 hover:text-slate-300 transition cursor-pointer"
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
                    className={`relative w-10 h-10 rounded-xl flex items-center justify-center text-sm font-bold transition cursor-pointer ${
                      currentWorkspaceId === ws.id
                        ? 'bg-purple-600 text-white'
                        : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
                    }`}
                    title={`${ws.name} · ${instanceLabel(group.instance.url)}${showBadge ? ` · ${unread} unread` : ''}`}
                  >
                    {ws.name.charAt(0).toUpperCase()}
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
                  className="w-10 h-10 rounded-xl flex items-center justify-center text-sm font-bold transition cursor-pointer opacity-40 hover:opacity-70 bg-slate-700 text-slate-400 relative"
                  title={`${ws.name} (deleted) · ${instanceLabel(group.instance.url)}`}
                >
                  <span className="line-through">{ws.name.charAt(0).toUpperCase()}</span>
                </button>
              ))}

              <button
                onClick={() => openCreateModal(group.instance.url)}
                className="w-10 h-10 rounded-xl flex items-center justify-center bg-slate-800 text-slate-400 hover:bg-slate-700 hover:text-white transition cursor-pointer"
                title={`Create workspace on ${instanceLabel(group.instance.url)}`}
              >
                <Plus className="w-4 h-4" />
              </button>
            </div>
          </div>
        ))}

        {groups.length > 0 && <div className="w-8 h-px bg-slate-700 my-1" />}

        <button
          onClick={onAddInstance}
          className="w-10 h-10 rounded-xl flex items-center justify-center bg-slate-800 text-slate-400 hover:bg-slate-700 hover:text-white transition cursor-pointer"
          title="Add instance"
        >
          <ServerCrash className="w-4 h-4" />
        </button>
      </div>

      {showNewWs && (
        <Modal title="Create Workspace" onClose={() => setShowNewWs(false)} dataQa="create-workspace-modal">
          <form onSubmit={handleCreate}>
            <h2 className="text-lg font-bold mb-1">Create Workspace</h2>
            <p className="text-xs text-slate-400 mb-4">{instanceLabel(newWsInstanceUrl)}</p>
            <input
              type="text"
              value={newWsName}
              onChange={(e) => setNewWsName(e.target.value)}
              placeholder="Workspace name"
              className="w-full px-4 py-3 bg-slate-700/50 border border-slate-600 rounded-lg text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-purple-500 mb-4"
              required
            />
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setShowNewWs(false)}
                className="px-4 py-2 text-slate-400 hover:text-white transition"
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
