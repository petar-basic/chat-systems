import { useEffect } from 'react';
import { Phone, PhoneOff } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { Avatar } from '@/shared/components/Avatar/Avatar';
import { useHuddleStore, type IncomingCall } from '@/stores/huddle';
import { useWorkspaceStore } from '@/stores/workspace';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';
import { playNotificationSound } from '@/lib/notifications';

export function IncomingCallRing({ call }: { call: IncomingCall }) {
  const { getUser } = useUserCache();
  const caller = getUser(call.fromUserId);
  const callerName = displayNameOf(caller?.display_name);

  useEffect(() => {
    playNotificationSound();
    const interval = window.setInterval(playNotificationSound, 3000);
    return () => window.clearInterval(interval);
  }, []);

  const decline = () => useHuddleStore.getState().removeIncomingCall(call.huddleId);

  const accept = () => {
    const selfUserId = useWorkspaceStore.getState().currentUserId;
    if (!selfUserId) {
      decline();
      return;
    }
    const store = useHuddleStore.getState();
    store.removeIncomingCall(call.huddleId);
    store.setActive({
      huddleId: call.huddleId,
      workspaceId: call.workspaceId,
      instanceUrl: call.instanceUrl,
      selfUserId,
      scope: { kind: 'dm', partnerId: call.fromUserId },
    });
  };

  return (
    <Modal
      title="Incoming huddle"
      onClose={decline}
      size="sm"
      dataQa="incoming-call"
      actions={
        <>
          <button
            onClick={decline}
            className="flex h-9 items-center gap-2 rounded-lg bg-raised px-4 text-sm font-medium text-fg transition hover:bg-elevated cursor-pointer"
          >
            <PhoneOff className="w-4 h-4" /> Decline
          </button>
          <button
            onClick={accept}
            data-autofocus
            className="flex h-9 items-center gap-2 rounded-lg bg-green-600 px-4 text-sm font-medium text-white transition hover:bg-green-500 cursor-pointer"
          >
            <Phone className="w-4 h-4" /> Join
          </button>
        </>
      }
    >
      <div className="flex items-center gap-3 py-2">
        <Avatar userId={call.fromUserId} name={callerName} avatarUrl={caller?.avatar_url} size="lg" />
        <div className="min-w-0">
          <p className="truncate text-base font-semibold text-fg">{callerName}</p>
          <p className="text-sm text-muted">is starting a huddle…</p>
        </div>
      </div>
    </Modal>
  );
}
