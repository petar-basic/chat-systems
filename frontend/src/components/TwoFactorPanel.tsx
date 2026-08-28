import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ShieldCheck, ShieldOff, Copy, Check } from 'lucide-react';
import { api } from '../lib/api';
import { toUserMessage } from '@/lib/errors';
import { toast } from '@/shared/components/Toast';

interface TotpStatus {
  enrolled: boolean;
  recovery_codes_remaining: number;
  required: boolean;
}

interface Enrolment {
  secret: string;
  provisioning_uri: string;
}

export default function TwoFactorPanel() {
  const queryClient = useQueryClient();
  const [enrolment, setEnrolment] = useState<Enrolment | null>(null);
  const [code, setCode] = useState('');
  const [recoveryCodes, setRecoveryCodes] = useState<string[] | null>(null);
  const [disabling, setDisabling] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { data: status } = useQuery({
    queryKey: ['totp-status'],
    queryFn: () => api.get<TotpStatus>('/auth/totp'),
  });

  const refresh = () => queryClient.invalidateQueries({ queryKey: ['totp-status'] });

  const start = useMutation({
    mutationFn: () => api.post<Enrolment>('/auth/totp/enrol', {}),
    onSuccess: (data) => {
      setError(null);
      setEnrolment(data);
    },
    onError: (e) => setError(toUserMessage(e)),
  });

  const confirm = useMutation({
    mutationFn: () => api.post<{ recovery_codes: string[] }>('/auth/totp/confirm', { code }),
    onSuccess: (data) => {
      setError(null);
      setEnrolment(null);
      setCode('');
      setRecoveryCodes(data.recovery_codes);
      void refresh();
    },
    onError: (e) => setError(toUserMessage(e)),
  });

  const disable = useMutation({
    mutationFn: () => api.post('/auth/totp/disable', { code }),
    onSuccess: () => {
      setError(null);
      setDisabling(false);
      setCode('');
      toast.success('Two-factor authentication is off.');
      void refresh();
    },
    onError: (e) => setError(toUserMessage(e)),
  });

  const copySecret = async () => {
    if (!enrolment) return;
    await navigator.clipboard.writeText(enrolment.secret);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div data-qa="two-factor">
      <div className="flex items-start justify-between gap-4 mb-3">
        <div>
          <h3 className="text-sm font-medium text-fg flex items-center gap-2">
            <ShieldCheck className="w-4 h-4 text-accent" />
            Two-factor authentication
          </h3>
          <p className="text-xs text-muted mt-1">
            {status?.enrolled
              ? `On. ${status.recovery_codes_remaining} recovery codes left.`
              : 'A stolen password alone is not enough to sign in as you.'}
          </p>
        </div>
        {status && !status.enrolled && !enrolment && (
          <button
            type="button"
            onClick={() => start.mutate()}
            disabled={start.isPending}
            data-qa="totp-start"
            className="px-3 py-2 text-sm bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white rounded-lg transition cursor-pointer whitespace-nowrap"
          >
            Set up
          </button>
        )}
        {status?.enrolled && !disabling && (
          <button
            type="button"
            onClick={() => setDisabling(true)}
            disabled={status.required}
            title={status.required ? 'This instance requires a second factor for admins' : undefined}
            data-qa="totp-disable"
            className="flex items-center gap-1.5 px-3 py-2 text-sm text-fg-dim hover:text-danger disabled:text-faint disabled:cursor-not-allowed transition cursor-pointer whitespace-nowrap"
          >
            <ShieldOff className="w-4 h-4" />
            Turn off
          </button>
        )}
      </div>

      {error && (
        <div className="text-sm text-danger bg-red-500/10 border border-red-500/30 rounded-lg px-3 py-2 mb-3">
          {error}
        </div>
      )}

      {enrolment && (
        <div className="space-y-3">
          <p className="text-xs text-muted">
            Add this to your authenticator app, then enter the code it shows. Nothing changes until you do —
            an enrolment you never finished cannot lock you out.
          </p>
          <div className="flex items-center gap-2">
            <code className="flex-1 px-3 py-2 bg-app border border-line rounded-lg text-xs text-fg-soft break-all">
              {enrolment.secret}
            </code>
            <button
              type="button"
              onClick={copySecret}
              aria-label="Copy the setup key"
              className="p-2 text-muted hover:text-fg transition cursor-pointer"
            >
              {copied ? <Check className="w-4 h-4 text-success" /> : <Copy className="w-4 h-4" />}
            </button>
          </div>
          <a
            href={enrolment.provisioning_uri}
            className="inline-block text-xs text-accent hover:text-accent-soft"
          >
            Open in your authenticator app
          </a>
          <div className="flex gap-2">
            <input
              type="text"
              inputMode="numeric"
              autoComplete="one-time-code"
              value={code}
              onChange={(e) => setCode(e.target.value.trim())}
              placeholder="123456"
              data-qa="totp-code"
              className="flex-1 px-3 py-2.5 bg-raised/50 border border-line-strong rounded-lg text-fg text-sm tracking-widest focus:outline-none focus:ring-2 focus:ring-purple-500"
            />
            <button
              type="button"
              onClick={() => confirm.mutate()}
              disabled={confirm.isPending || code.length < 6}
              data-qa="totp-confirm"
              className="px-4 py-2.5 bg-purple-600 hover:bg-purple-500 disabled:bg-purple-600/50 text-white text-sm rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
            >
              Confirm
            </button>
          </div>
        </div>
      )}

      {recoveryCodes && (
        <div className="mt-3 space-y-2" data-qa="totp-recovery-codes">
          <p className="text-xs text-warning">
            Save these somewhere safe. They are shown once, and each one works once — they are the way back in
            if you lose the phone.
          </p>
          <div className="grid grid-cols-2 gap-2">
            {recoveryCodes.map((recoveryCode) => (
              <code
                key={recoveryCode}
                className="px-3 py-2 bg-app border border-line rounded-lg text-xs text-fg-soft text-center"
              >
                {recoveryCode}
              </code>
            ))}
          </div>
          <button
            type="button"
            onClick={() => setRecoveryCodes(null)}
            className="text-xs text-muted hover:text-fg transition cursor-pointer"
          >
            I have saved them
          </button>
        </div>
      )}

      {disabling && (
        <div className="flex gap-2">
          <input
            type="text"
            inputMode="numeric"
            autoComplete="one-time-code"
            value={code}
            onChange={(e) => setCode(e.target.value.trim())}
            placeholder="Current code"
            data-qa="totp-disable-code"
            className="flex-1 px-3 py-2.5 bg-raised/50 border border-line-strong rounded-lg text-fg text-sm tracking-widest focus:outline-none focus:ring-2 focus:ring-purple-500"
          />
          <button
            type="button"
            onClick={() => disable.mutate()}
            disabled={disable.isPending || code.length < 6}
            data-qa="totp-disable-confirm"
            className="px-4 py-2.5 bg-red-600 hover:bg-red-500 disabled:bg-red-600/50 text-white text-sm rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
          >
            Turn off
          </button>
          <button
            type="button"
            onClick={() => {
              setDisabling(false);
              setCode('');
            }}
            className="px-3 py-2.5 text-sm text-muted hover:text-fg transition cursor-pointer"
          >
            Cancel
          </button>
        </div>
      )}
    </div>
  );
}
