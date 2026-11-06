import { useState } from 'react';
import { Check, Copy } from 'lucide-react';
import { useTranslation } from 'react-i18next';

interface Props {
  label: string;
  value: string;
  valueQa?: string;
  copyQa?: string;
}

export function CopyField({ label, value, valueQa, copyQa }: Props) {
  const { t } = useTranslation('common');
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      setCopied(false);
    }
  };

  return (
    <div className="mt-2">
      <div className="text-2xs uppercase tracking-wider text-muted mb-1">{label}</div>
      <div className="flex items-center gap-1.5">
        <code
          data-qa={valueQa}
          className="flex-1 min-w-0 truncate text-xs bg-app/70 border border-line rounded px-2 py-1.5 text-fg-soft"
        >
          {value}
        </code>
        <button
          onClick={copy}
          aria-label={t('copyNamed', { name: label })}
          data-qa={copyQa}
          className="p-1.5 rounded text-muted hover:text-fg hover:bg-raised transition cursor-pointer"
        >
          {copied ? <Check className="w-3.5 h-3.5 text-success" /> : <Copy className="w-3.5 h-3.5" />}
        </button>
      </div>
    </div>
  );
}
