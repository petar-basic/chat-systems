import { Hash } from 'lucide-react';
import { ROUTES, EmptyLabels } from '@/shared/constants';
import { ConnectionBanner } from '@/shared/components/ConnectionBanner/ConnectionBanner';
import { QuickSwitcher } from '@/features/navigation';
import { WorkspaceSidebar, WorkspaceRightPanels, useWorkspaceController } from '@/features/workspace';
import { ChannelSidebar, ChannelHeader } from '@/features/channel';
import { MessageList, MessageInput } from '@/features/messaging';
import DmView from './DmView';
import AddInstancePanel from '../components/AddInstancePanel';
import UserProfilePanel from '../components/UserProfilePanel';
import TypingIndicator from '../components/TypingIndicator';

export default function WorkspacePage() {
  const c = useWorkspaceController();
  const { panel, currentWorkspace, currentChannel, user } = c;

  return (
    <div className="h-dvh flex bg-slate-900 text-white relative">
      <div
        className={`flex shrink-0 max-lg:fixed max-lg:inset-y-0 max-lg:left-0 max-lg:z-40 max-lg:shadow-2xl transition-transform ${
          c.mobileNavOpen ? 'max-lg:translate-x-0' : 'max-lg:-translate-x-full'
        }`}
      >
        <WorkspaceSidebar
          workspaces={c.workspaces}
          deletedWorkspaces={c.deletedWorkspaces}
          currentWorkspaceId={currentWorkspace?.id}
          onSelectWorkspace={c.handleSelectWorkspace}
          onCreateWorkspace={c.handleCreateWorkspace}
          onAddInstance={() => c.setShowAddInstance(true)}
        />

        <ChannelSidebar
          currentWorkspace={currentWorkspace}
          channels={c.channels}
          currentChannel={currentChannel}
          unreadChannels={c.unreadChannels}
          mentionChannels={c.mentionChannels}
          mutedChannels={c.mutedChannels}
          onToggleMute={(channelId, muted) => c.setChannelMuted({ channelId, muted })}
          workspaceMembers={c.workspaceMembers}
          currentUserId={user?.id}
          user={user || null}
          dmConversations={c.dmConversations}
          currentDmPartnerId={c.currentDmPartnerId}
          unreadDmPartners={c.unreadDmPartners}
          onSelectChannel={c.handleSelectChannel}
          onCreateChannel={c.handleCreateChannel}
          onOpenDm={c.handleOpenDm}
          onOpenMembers={() => panel.toggle('members')}
          onOpenSettings={() => panel.toggle('settings')}
          onOpenProfile={() => c.setShowProfile(true)}
          onOpenNotifications={() => panel.toggle('notifications')}
          onLogout={() => c.logout.mutate()}
        />
      </div>

      {c.mobileNavOpen && (
        <div
          className="fixed inset-0 bg-black/50 z-30 lg:hidden"
          onClick={() => c.setMobileNavOpen(false)}
          aria-hidden
        />
      )}

      {c.showAddInstance && <AddInstancePanel onClose={() => c.setShowAddInstance(false)} />}

      {c.currentDmPartnerId && currentWorkspace && user ? (
        <DmView
          workspaceId={currentWorkspace.id}
          instanceUrl={currentWorkspace.instanceUrl}
          partnerId={c.currentDmPartnerId}
          currentUserId={user.id}
          onClose={() => c.navigate(ROUTES.workspace(currentWorkspace.id))}
          onOpenNav={() => c.setMobileNavOpen(true)}
        />
      ) : (
        <main className="flex-1 flex flex-col min-w-0" aria-label="Conversation">
          <ConnectionBanner instanceUrl={currentWorkspace?.instanceUrl} />
          {currentWorkspace?.deleted_at && (
            <div className="bg-yellow-500/10 border-b border-yellow-500/30 px-4 py-2 flex items-center justify-between shrink-0">
              <p className="text-sm text-yellow-300">
                This workspace has been soft-deleted and is not visible to regular members.
              </p>
              <button
                onClick={() => {
                  if (!currentWorkspace.instanceUrl) return;
                  c.restoreWorkspace.mutate({
                    workspaceId: currentWorkspace.id,
                    instanceUrl: currentWorkspace.instanceUrl,
                  });
                }}
                disabled={c.restoreWorkspace.isPending}
                className="ml-4 px-3 py-1 text-xs bg-yellow-500/20 hover:bg-yellow-500/40 border border-yellow-500/40 text-yellow-300 rounded-lg transition cursor-pointer disabled:opacity-50 shrink-0"
              >
                {c.restoreWorkspace.isPending ? 'Restoring...' : 'Restore Workspace'}
              </button>
            </div>
          )}

          <ChannelHeader
            channel={currentChannel}
            showSearch={panel.active?.kind === 'search'}
            showPins={panel.active?.kind === 'pins'}
            showChannelMembers={panel.active?.kind === 'channelMembers'}
            onToggleSearch={() => panel.toggle('search')}
            onTogglePins={() => panel.toggle('pins')}
            onToggleChannelMembers={() => panel.toggle('channelMembers')}
            onOpenNav={() => c.setMobileNavOpen(true)}
          />

          {currentChannel && (
            <MessageList
              channelId={currentChannel.id}
              members={c.workspaceMembers}
              channels={c.channels}
              onThreadOpen={panel.openThread}
              highlightMessageId={c.urlMessageId}
              onTargetMessageFound={c.handleTargetMessageFound}
            />
          )}

          {currentChannel && user && (
            <TypingIndicator channelId={currentChannel.id} currentUserId={user.id} />
          )}

          {currentChannel && (
            <MessageInput
              key={currentChannel.id}
              channelName={currentChannel.name}
              draftKey={currentChannel.id}
              members={c.workspaceMembers}
              channels={c.channels}
              onSend={c.handleSend}
              onFileUpload={c.handleFileUpload}
              onTyping={c.handleTyping}
              uploading={c.uploading}
            />
          )}

          {!currentChannel && (
            <div className="flex-1 flex flex-col items-center justify-center text-slate-400 gap-3 px-6 text-center">
              {c.channels.length === 0 ? (
                <>
                  <Hash className="w-12 h-12 text-slate-600" />
                  <p className="text-lg font-medium">{EmptyLabels.NoChannels}</p>
                  <p className="text-sm">{EmptyLabels.NoChannelsHint}</p>
                </>
              ) : (
                <div className="w-6 h-6 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
              )}
            </div>
          )}
        </main>
      )}

      <WorkspaceRightPanels
        panel={panel.active}
        currentWorkspace={currentWorkspace}
        currentChannel={currentChannel}
        workspaceMembers={c.workspaceMembers}
        channels={c.channels}
        onClose={panel.close}
        onNavigateToMessage={c.handleNavigateToMessage}
      />

      {c.showProfile && <UserProfilePanel onClose={() => c.setShowProfile(false)} />}

      {c.quickSwitcherOpen && currentWorkspace && (
        <QuickSwitcher
          channels={c.channels}
          members={c.workspaceMembers}
          currentUserId={user?.id}
          onSelectChannel={c.handleSelectChannel}
          onSelectDm={c.handleOpenDm}
          onClose={() => c.setQuickSwitcherOpen(false)}
        />
      )}
    </div>
  );
}
