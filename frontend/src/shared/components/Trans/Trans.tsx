import type { ComponentType } from 'react';
import type { TOptions } from 'i18next';
import { Trans as BaseTrans } from 'react-i18next';
import { escapeText } from '@/lib/i18n';

type Props = { tOptions?: TOptions; shouldUnescape?: boolean } & Record<string, unknown>;

const Base = BaseTrans as unknown as ComponentType<Props>;

function EscapingTrans({ tOptions, ...props }: Props) {
  return (
    <Base
      {...props}
      tOptions={{
        ...tOptions,
        interpolation: { ...tOptions?.interpolation, escapeValue: true, escape: escapeText },
      }}
      shouldUnescape
    />
  );
}

export const Trans = EscapingTrans as unknown as typeof BaseTrans;
