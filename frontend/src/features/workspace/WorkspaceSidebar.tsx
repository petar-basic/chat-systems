import { useState, type FormEvent } from 'react';
import { Button } from '@/shared/components/Button/Button';
import { TextInput } from '@/shared/components/Form/TextInput';
import { Plus, ServerCrash, RefreshCw, Upload } from 'lucide-react';
import type { Workspace } from '@/stores/workspace';
import { useInstanceStore } from '@/stores/instances';
import { useWsStatusStore } from '@/stores/wsStatus';
import { instanceManager } from '@/lib/instances';
import { Modal } from '@/shared/components/Modal/Modal';
import { useWorkspaceUnreadCounts } from '@/features/notifications';
import { avatarColorFor } from '@/lib/userHelpers';
import { workspaceInitials } from './workspaceTile';
import { WorkspaceIcon } from './components/WorkspaceIcon';

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
                        aria-label="Retry connection"
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
                  <div key={ws.id} className="relative">
                    <button
                      onClick={() => onSelectWorkspace(ws)}
                      aria-label={ws.name}
                      aria-current={currentWorkspaceId === ws.id ? 'true' : undefined}
                      data-qa="workspace-tile"
                      className={`w-10 h-10 rounded-xl overflow-hidden text-sm font-bold text-white transition cursor-pointer ${
                        currentWorkspaceId === ws.id
                          ? 'ring-2 ring-purple-400 ring-offset-2 ring-offset-rail'
                          : 'opacity-80 hover:opacity-100'
                      }`}
                      title={`${ws.name} · ${instanceLabel(group.instance.url)}${showBadge ? ` · ${unread} unread` : ''}`}
                    >
                      <WorkspaceIcon
                        workspace={ws}
                        className="w-full h-full"
                        initialsClassName={avatarColorFor(ws.id)}
                      />
                    </button>
                    {/* Outside the button: the icon needs `overflow-hidden` to
                        stay in its rounded box, which would clip a badge that
                        is meant to sit on the corner. */}
                    {showBadge && (
                      <span
                        aria-label={`${unread} unread notifications`}
                        className="pointer-events-none absolute -top-1 -right-1 min-w-4 h-4 px-1 bg-red-500 text-white text-[9px] font-bold rounded-full flex items-center justify-center leading-none ring-2 ring-rail"
                      >
                        {unread > 99 ? '99+' : unread}
                      </span>
                    )}
                  </div>
                );
              })}

              {group.deletedWorkspaces.map((ws) => (
                <button
                  key={ws.id}
                  onClick={() => onSelectWorkspace(ws)}
                  className="w-10 h-10 rounded-xl flex items-center justify-center text-sm font-bold transition cursor-pointer opacity-40 hover:opacity-70 bg-raised text-muted relative"
                  title={`${ws.name} (deleted) · ${instanceLabel(group.instance.url)}`}
                >
                  <span className="line-through">{workspaceInitials(ws.name)}</span>
                </button>
              ))}

              <button
                onClick={() => openCreateModal(group.instance.url)}
                className="w-10 h-10 rounded-xl flex items-center justify-center bg-surface text-muted hover:bg-raised hover:text-fg transition cursor-pointer"
                title={`Create workspace on ${instanceLabel(group.instance.url)}`}
                aria-label={`Create workspace on ${instanceLabel(group.instance.url)}`}
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
          aria-label="Add instance"
        >
          <ServerCrash className="w-4 h-4" />
        </button>
      </div>

      {showNewWs && (
        <Modal
          title="Create workspace"
          description={instanceLabel(newWsInstanceUrl)}
          onClose={() => setShowNewWs(false)}
          size="sm"
          dataQa="create-workspace-modal"
          onSubmit={handleCreate}
          dismissLabel="Cancel"
          actions={
            <Button type="submit" variant="primary">
              Create
            </Button>
          }
        >
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
              <span className="block text-xs text-muted">Bring a workspace export in, with its history</span>
            </span>
          </button>

          <TextInput
            label="Workspace name"
            value={newWsName}
            onChange={(e) => setNewWsName(e.target.value)}
            placeholder="e.g. Acme"
            required
            data-autofocus
          />
        </Modal>
      )}
    </>
  );
}
