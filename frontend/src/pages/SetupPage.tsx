import { useState } from 'react';
import { useNavigate } from 'react-router';
import { Rocket } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import { Spinner } from '@/shared/components/Spinner/Spinner';
import { useInstanceInfo } from '@/features/instance/hooks/useInstanceInfo';
import { SetupForm } from '@/features/setup/SetupForm';
import { TestEmailCheck } from '@/features/setup/TestEmailCheck';
import { toUserMessage } from '@/lib/errors';

const origin = window.location.origin;

export default function SetupPage() {
  const navigate = useNavigate();
  const info = useInstanceInfo(origin);
  const [workspaceId, setWorkspaceId] = useState<string | null>(null);

  const body = workspaceId ? (
    <div className="space-y-6" data-qa="setup-done">
      <p className="text-fg">You are the instance admin, and your workspace is ready.</p>
      <TestEmailCheck instanceUrl={origin} />
      <Button
        variant="primary"
        fullWidth
        onClick={() => navigate(`/app/${workspaceId}`, { replace: true })}
        data-qa="setup-continue"
      >
        Open the workspace
      </Button>
    </div>
  ) : info.isPending ? (
    <div className="flex justify-center py-8">
      <Spinner className="text-accent" />
    </div>
  ) : info.isError ? (
    <p className="text-sm text-danger">{toUserMessage(info.error, 'Could not reach this instance')}</p>
  ) : info.data.setup_required ? (
    <SetupForm onDone={setWorkspaceId} />
  ) : (
    <div className="space-y-6 text-center" data-qa="setup-not-needed">
      <p className="text-muted">This instance is already set up.</p>
      <Button variant="primary" fullWidth onClick={() => navigate('/add-instance', { replace: true })}>
        Sign in
      </Button>
    </div>
  );

  return (
    <div className="min-h-screen bg-gradient-to-br from-app via-accent-deep to-app flex items-center justify-center p-4">
      <div className="w-full max-w-md">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 bg-purple-600 rounded-2xl mb-4">
            <Rocket className="w-8 h-8 text-fg" />
          </div>
          <h1 className="text-3xl font-bold text-fg">Set up {info.data?.name ?? 'Chat Systems'}</h1>
          <p className="text-muted mt-2">Create the first admin account and workspace.</p>
        </div>
        <div className="bg-surface/50 backdrop-blur-xl border border-line rounded-2xl p-8 shadow-2xl">
          {body}
        </div>
      </div>
    </div>
  );
}
