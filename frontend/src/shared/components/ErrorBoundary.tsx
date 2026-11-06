import { Component } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/shared/components/Button/Button';
import type { ErrorInfo, ReactNode } from 'react';
import { logger } from '@/lib/logger';

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
}

function Crashed({ onReload }: { onReload: () => void }) {
  const { t } = useTranslation('common');
  return (
    <div className="h-screen flex flex-col items-center justify-center bg-app text-fg gap-4 px-6 text-center">
      <div className="text-lg font-semibold">{t('crashed.title')}</div>
      <Button variant="primary" onClick={onReload}>
        {t('crashed.reload')}
      </Button>
    </div>
  );
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    logger.error('ErrorBoundary', 'componentDidCatch', { error, componentStack: errorInfo.componentStack });
  }

  private handleReload = () => {
    window.location.reload();
  };

  render() {
    if (this.state.hasError) {
      return <Crashed onReload={this.handleReload} />;
    }

    return this.props.children;
  }
}
