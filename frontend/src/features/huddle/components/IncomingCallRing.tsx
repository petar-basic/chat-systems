import { useEffect } from 'react';
import { Phone, PhoneOff } from 'lucide-react';
import { Modal } from '@/shared/components/Modal/Modal';
import { useHuddleStore, type IncomingCall } from '@/stores/huddle';
import { useWorkspaceStore } from '@/stores/workspace';
import { useUserCache } from '@/stores/users';
import { displayNameOf } from '@/lib/userHelpers';
import { playNotificationSound } from '@/lib/notifications';

export function IncomingCallRing({ call }: { call: IncomingCall }) {
  const { getUser } = useUserCache();
  const callerName = displayNameOf(getUser(call.fromUserId)?.display_name);

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
    <Modal title="Incoming huddle" onClose={decline}>
      <div className="flex flex-col items-center gap-5 py-2">
        <div className="text-center">
          <p className="text-lg font-semibold text-white">{callerName}</p>
          <p className="text-sm text-slate-400">is starting a huddle…</p>
        </div>
        <div className="flex gap-3">
          <button
            onClick={decline}
            className="flex items-center gap-2 px-4 py-2 rounded-xl bg-slate-700 text-white hover:bg-slate-600 transition cursor-pointer"
          >
            <PhoneOff className="w-4 h-4" /> Decline
          </button>
          <button
            onClick={accept}
            className="flex items-center gap-2 px-4 py-2 rounded-xl bg-green-600 text-white hover:bg-green-500 transition cursor-pointer"
          >
            <Phone className="w-4 h-4" /> Join
          </button>
        </div>
      </div>
    </Modal>
  );
}
