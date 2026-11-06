import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/shared/components/Button/Button';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ShieldCheck, ShieldOff, Copy, Check } from 'lucide-react';
import { QRCodeSVG } from 'qrcode.react';
import { api } from '@/lib/api';
import { toUserMessage } from '@/lib/errors';
import { toast } from '@/shared/components/Toast';
import type { components } from '@/api/schema';
import { TextInput } from '@/shared/components/Form/TextInput';

type Enrolment = components['schemas']['TotpEnrolment'];

export default function TwoFactorPanel() {
  const { t } = useTranslation('profile');
  const queryClient = useQueryClient();
  const [enrolment, setEnrolment] = useState<Enrolment | null>(null);
  const [code, setCode] = useState('');
  const [recoveryCodes, setRecoveryCodes] = useState<string[] | null>(null);
  const [disabling, setDisabling] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { data: status } = useQuery({
    queryKey: ['totp-status'],
    queryFn: () => api.typed((c) => c.GET('/auth/totp')),
  });

  const refresh = () => queryClient.invalidateQueries({ queryKey: ['totp-status'] });

  const start = useMutation({
    mutationFn: () => api.typed((c) => c.POST('/auth/totp/enrol')),
    onSuccess: (data) => {
      setError(null);
      setEnrolment(data);
    },
    onError: (e) => setError(toUserMessage(e)),
  });

  const confirm = useMutation({
    mutationFn: () => api.typed((c) => c.POST('/auth/totp/confirm', { body: { code } })),
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
    mutationFn: () => api.typed((c) => c.POST('/auth/totp/disable', { body: { code } })),
    onSuccess: () => {
      setError(null);
      setDisabling(false);
      setCode('');
      toast.success(t('twoFactor.turnedOff'));
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
            {t('twoFactor.title')}
          </h3>
          <p className="text-xs text-muted mt-1">
            {status?.enrolled
              ? t('twoFactor.on', { count: status.recovery_codes_remaining })
              : t('twoFactor.off')}
          </p>
        </div>
        {status && !status.enrolled && !enrolment && (
          <Button
            variant="primary"

            onClick={() => start.mutate()}
            disabled={start.isPending}
            data-qa="totp-start"
          >
            {t('twoFactor.setUp')}
          </Button>
        )}
        {status?.enrolled && !disabling && (
          <button
            type="button"
            onClick={() => setDisabling(true)}
            disabled={status.required}
            data-tooltip={status.required ? t('twoFactor.required') : undefined}
            data-qa="totp-disable"
            className="flex items-center gap-1.5 px-3 py-2 text-sm text-fg-dim hover:text-danger disabled:text-faint disabled:cursor-not-allowed transition cursor-pointer whitespace-nowrap"
          >
            <ShieldOff className="w-4 h-4" />
            {t('twoFactor.turnOff')}
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
          <p className="text-xs text-muted">{t('twoFactor.scan')}</p>
          <div className="inline-block rounded-lg bg-white p-3" data-qa="totp-qr">
            <QRCodeSVG value={enrolment.provisioning_uri} size={176} title={t('twoFactor.qr')} />
          </div>
          <div className="flex items-center gap-2">
            <code className="flex-1 px-3 py-2 bg-app border border-line rounded-lg text-xs text-fg-soft break-all">
              {enrolment.secret}
            </code>
            <button
              type="button"
              onClick={copySecret}
              aria-label={t('twoFactor.copyKey')}
              data-tooltip={t('twoFactor.copyKey')}
              className="p-2 text-muted hover:text-fg transition cursor-pointer"
            >
              {copied ? <Check className="w-4 h-4 text-success" /> : <Copy className="w-4 h-4" />}
            </button>
          </div>
          <a
            href={enrolment.provisioning_uri}
            className="inline-block text-xs text-accent hover:text-accent-soft"
          >
            {t('twoFactor.openInApp')}
          </a>
          <div className="flex gap-2">
            <TextInput
              label={t('twoFactor.code')}
              labelHidden
              inputMode="numeric"
              autoComplete="one-time-code"
              value={code}
              onChange={(e) => setCode(e.target.value.trim())}
              placeholder="123456"
              dataQa="totp-code"
              className="flex-1"
            />
            <Button
              variant="primary"

              onClick={() => confirm.mutate()}
              disabled={confirm.isPending || code.length < 6}
              data-qa="totp-confirm"
            >
              {t('twoFactor.confirm')}
            </Button>
          </div>
        </div>
      )}

      {recoveryCodes && (
        <div className="mt-3 space-y-2" data-qa="totp-recovery-codes">
          <p className="text-xs text-warning">{t('twoFactor.recovery')}</p>
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
            {t('twoFactor.savedThem')}
          </button>
        </div>
      )}

      {disabling && (
        <div className="flex gap-2">
          <TextInput
            label={t('twoFactor.currentCode')}
            labelHidden
            inputMode="numeric"
            autoComplete="one-time-code"
            value={code}
            onChange={(e) => setCode(e.target.value.trim())}
            placeholder={t('twoFactor.currentCodePlaceholder')}
            dataQa="totp-disable-code"
            className="flex-1"
          />
          <button
            type="button"
            onClick={() => disable.mutate()}
            disabled={disable.isPending || code.length < 6}
            data-qa="totp-disable-confirm"
            className="px-4 py-2.5 bg-red-600 hover:bg-red-500 disabled:bg-red-600/50 text-white text-sm rounded-lg transition cursor-pointer disabled:cursor-not-allowed"
          >
            {t('twoFactor.turnOff')}
          </button>
          <button
            type="button"
            onClick={() => {
              setDisabling(false);
              setCode('');
            }}
            className="px-3 py-2.5 text-sm text-muted hover:text-fg transition cursor-pointer"
          >
            {t('twoFactor.cancel')}
          </button>
        </div>
      )}
    </div>
  );
}
