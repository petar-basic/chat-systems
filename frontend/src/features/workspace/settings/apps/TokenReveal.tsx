import { CopyField } from '@/shared/components/CopyField/CopyField';

interface Props {
  name: string;
  token: string;
  onDismiss: () => void;
}

export function TokenReveal({ name, token, onDismiss }: Props) {
  return (
    <div className="rounded-lg border border-warning/40 bg-warning/5 p-3" data-qa="bot-app-token">
      <p className="text-sm text-fg-soft">
        Copy {name}'s token now. It is not shown again, and nobody can read it back later.
      </p>
      <CopyField label="Bot token" value={token} valueQa="bot-app-token-value" copyQa="bot-app-token-copy" />
      <button
        onClick={onDismiss}
        data-qa="bot-app-token-dismiss"
        className="mt-2 text-xs text-muted hover:text-fg transition cursor-pointer"
      >
        I have stored it
      </button>
    </div>
  );
}
