import type { ReactNode } from 'react';
import { AlertCircle } from 'lucide-react';
import { ActionLabels, GENERIC_ERROR_MESSAGE } from '@/shared/constants';

interface QueryStateProps {
  isLoading: boolean;
  isError: boolean;
  isEmpty?: boolean;
  onRetry?: () => void;
  errorMessage?: string;
  empty?: ReactNode;
  children: ReactNode;
}

const centered =
  'flex-1 overflow-y-auto px-4 py-4 flex flex-col items-center justify-center text-slate-400 text-center';

export function QueryState({
  isLoading,
  isError,
  isEmpty,
  onRetry,
  errorMessage,
  empty,
  children,
}: QueryStateProps) {
  if (isLoading) {
    return (
      <div className={centered} data-qa="query-loading">
        <div className="w-6 h-6 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
      </div>
    );
  }

  if (isError) {
    return (
      <div className={centered} data-qa="query-error">
        <AlertCircle className="w-10 h-10 mb-3 text-red-400/70" />
        <p className="text-sm font-medium text-slate-300">{errorMessage || GENERIC_ERROR_MESSAGE}</p>
        {onRetry && (
          <button
            onClick={onRetry}
            data-qa="query-retry"
            className="mt-3 px-3 py-1.5 text-xs bg-slate-700 hover:bg-slate-600 text-slate-200 rounded-lg transition"
          >
            {ActionLabels.Retry}
          </button>
        )}
      </div>
    );
  }

  if (isEmpty) {
    return (
      <div className={centered} data-qa="query-empty">
        {empty}
      </div>
    );
  }

  return <>{children}</>;
}
