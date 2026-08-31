import { Hash, Megaphone } from 'lucide-react';
import { ROUTES, EmptyLabels } from '@/shared/constants';
import { useUserCache } from '@/stores/users';
import { conversationTitle } from '@/lib/conversationHelpers';
import { ConnectionBanner } from '@/shared/components/ConnectionBanner/ConnectionBanner';
import InstallAppBanner from '@/components/InstallAppBanner';
import { QuickSwitcher } from '@/features/navigation';
import { WorkspaceSidebar, WorkspaceRightPanels, useWorkspaceController } from '@/features/workspace';
import { ChannelSidebar, ChannelHeader, ChannelBookmarksBar } from '@/features/channel';
import { MessageList, MessageInput } from '@/features/messaging';
import ConversationView from '@/features/messaging/ConversationView';
import ForwardMessageModal from '@/features/messaging/ForwardMessageModal';
import AddInstancePanel from '../components/AddInstancePanel';
import UserProfilePanel from '../components/UserProfilePanel';
import TypingIndicator from '../components/TypingIndicator';
import { useChannelModeration } from '@/features/channel/hooks/useChannelModeration';
import { useEscapeToClose } from '@/shared/hooks/useEscapeToClose';
import { useActiveHuddlesSync } from '@/features/huddle';

export default function WorkspacePage() {
  const c = useWorkspaceController();
  const { panel, currentWorkspace, currentChannel, user } = c;
  const { getUser } = useUserCache();
  const activeConversation = c.conversations.find((conv) => conv.id === c.currentConversationId);

  useActiveHuddlesSync(currentWorkspace?.id, currentWorkspace?.instanceUrl);

  const { canPost } = useChannelModeration(currentChannel?.id ?? null, currentChannel?.settings);

  // Every other panel and modal closes on Escape; the drawer did not.
  useEscapeToClose(() => c.setMobileNavOpen(false), c.mobileNavOpen);

  return (
    <div className="h-dvh flex flex-col bg-app text-fg">
      <InstallAppBanner />
      <div className="flex-1 flex min-h-0 relative">
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
            onImportFromSlack={() => panel.toggle('slackImport')}
            onAddInstance={() => c.setShowAddInstance(true)}
          />

          <ChannelSidebar
            currentWorkspace={currentWorkspace}
            channels={c.channels}
            currentChannel={currentChannel}
            unreadChannels={c.unreadChannels}
            unreadCounts={c.unreadCounts}
            mentionCounts={c.mentionCounts}
            mentionChannels={c.mentionChannels}
            mutedChannels={c.mutedChannels}
            onToggleMute={(channelId, muted) => c.setChannelMuted({ channelId, muted })}
            workspaceMembers={c.workspaceMembers}
            currentUserId={user?.id}
            user={user || null}
            conversations={c.conversations}
            currentConversationId={c.currentConversationId}
            unreadConversations={c.unreadConversations}
            onSelectChannel={c.handleSelectChannel}
            onCreateChannel={c.handleCreateChannel}
            onOpenConversation={c.handleOpenConversation}
            onOpenWith={c.handleOpenWith}
            onOpenMembers={() => panel.toggle('members')}
            onOpenSettings={() => panel.toggle('settings')}
            onOpenIntegrations={() => panel.toggle('integrations')}
            onOpenCustomEmoji={() => panel.toggle('customEmoji')}
            onOpenUserGroups={() => panel.toggle('userGroups')}
            onOpenAuditLog={() => panel.toggle('auditLog')}
            onOpenScheduled={() => panel.toggle('scheduled')}
            onOpenSaved={() => panel.toggle('saved')}
            onOpenSlackImport={() => panel.toggle('slackImport')}
            onOpenReminders={() => panel.toggle('reminders')}
            onOpenProfile={() => c.setShowProfile(true)}
            onOpenNotifications={() => panel.toggle('notifications')}
            onLogout={() => c.logout.mutate()}
          />
        </div>

        {c.mobileNavOpen && (
          <div
            className="fixed inset-0 bg-overlay/50 z-30 lg:hidden"
            onClick={() => c.setMobileNavOpen(false)}
            aria-hidden
          />
        )}

        {c.showAddInstance && <AddInstancePanel onClose={() => c.setShowAddInstance(false)} />}

        {activeConversation && currentWorkspace && user ? (
          <ConversationView
            workspaceId={currentWorkspace.id}
            instanceUrl={currentWorkspace.instanceUrl}
            conversationId={activeConversation.id}
            title={conversationTitle(activeConversation, user.id, (id) => getUser(id)?.display_name)}
            participantIds={activeConversation.participant_ids}
            kind={activeConversation.kind}
            currentUserId={user.id}
            onClose={() => c.navigate(ROUTES.workspace(currentWorkspace.id))}
            onOpenNav={() => c.setMobileNavOpen(true)}
            onOpenThread={(message) => panel.openConversationThread(activeConversation.id, message)}
            onSave={c.handleSaveConversationMessage}
            onForward={c.handleForwardConversationMessage}
          />
        ) : (
          <main className="flex-1 flex flex-col min-w-0" aria-label="Conversation">
            <ConnectionBanner instanceUrl={currentWorkspace?.instanceUrl} />
            {currentWorkspace?.deleted_at && (
              <div className="bg-yellow-500/10 border-b border-yellow-500/30 px-4 py-2 flex items-center justify-between shrink-0">
                <p className="text-sm text-warning">
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
                  className="ml-4 px-3 py-1 text-xs bg-yellow-500/20 hover:bg-yellow-500/40 border border-yellow-500/40 text-warning rounded-lg transition cursor-pointer disabled:opacity-50 shrink-0"
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
              <ChannelBookmarksBar
                channelId={currentChannel.id}
                instanceUrl={currentWorkspace?.instanceUrl}
              />
            )}

            {currentChannel && (
              <MessageList
                channelId={currentChannel.id}
                members={c.workspaceMembers}
                channels={c.channels}
                onThreadOpen={panel.openThread}
                onSave={c.handleSaveMessage}
                onForward={c.handleForwardMessage}
                highlightMessageId={c.urlMessageId}
                onTargetMessageFound={c.handleTargetMessageFound}
              />
            )}

            {currentChannel && user && (
              <TypingIndicator channelId={currentChannel.id} currentUserId={user.id} />
            )}

            {c.ephemeral && (
              <div
                data-qa="command-response"
                className="mx-4 mb-2 px-3 py-2 rounded-lg bg-surface/80 border border-line text-sm text-fg-dim flex items-start gap-2"
              >
                <span className="flex-1 whitespace-pre-wrap">{c.ephemeral}</span>
                <button
                  type="button"
                  onClick={c.dismissEphemeral}
                  aria-label="Dismiss"
                  className="text-subtle hover:text-fg transition cursor-pointer"
                >
                  ×
                </button>
              </div>
            )}

            {currentChannel && !canPost && (
              <div
                data-qa="channel-read-only"
                className="mx-4 mb-4 px-4 py-3 rounded-lg bg-surface/60 border border-line text-sm text-muted flex items-center gap-2"
              >
                <Megaphone className="w-4 h-4 shrink-0 text-warning" />
                Only admins can post in this channel.
              </div>
            )}

            {currentChannel && canPost && (
              <MessageInput
                key={currentChannel.id}
                workspaceId={currentWorkspace?.id}
                instanceUrl={currentWorkspace?.instanceUrl}
                scheduleTarget={{ channelId: currentChannel.id }}
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
              <div className="flex-1 flex flex-col items-center justify-center text-muted gap-3 px-6 text-center">
                {c.channels.length === 0 ? (
                  <>
                    <Hash className="w-12 h-12 text-faint" />
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
          conversations={c.conversations}
          onClose={panel.close}
          currentUserId={user?.id}
          onNavigateToMessage={c.handleNavigateToMessage}
          onOpenConversation={c.handleOpenConversation}
        />

        {c.forwarding && currentWorkspace && user && (
          <ForwardMessageModal
            source={c.forwarding}
            channels={c.channels}
            conversations={c.conversations}
            currentUserId={user.id}
            instanceUrl={currentWorkspace.instanceUrl}
            onClose={c.dismissForward}
          />
        )}

        {c.showProfile && <UserProfilePanel onClose={() => c.setShowProfile(false)} />}

        {c.quickSwitcherOpen && currentWorkspace && (
          <QuickSwitcher
            channels={c.channels}
            members={c.workspaceMembers}
            currentUserId={user?.id}
            onSelectChannel={c.handleSelectChannel}
            onSelectDm={(userId) => void c.handleOpenWith([userId])}
            onClose={() => c.setQuickSwitcherOpen(false)}
          />
        )}
      </div>
    </div>
  );
}
