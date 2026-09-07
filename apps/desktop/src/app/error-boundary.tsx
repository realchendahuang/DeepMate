import { Component, type ErrorInfo, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/shared/ui/button";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
  stack: string | null;
}

// Last-resort guard: a rendering error shows a recoverable panel instead of
// blanking the whole window. Pages can still add their own boundaries for a
// friendlier, scoped fallback.
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, stack: null };

  static getDerivedStateFromError(error: Error): State {
    return { error, stack: null };
  }

  componentDidCatch(_error: Error, info: ErrorInfo) {
    this.setState({ stack: info.componentStack ?? null });
  }

  render() {
    if (!this.state.error) return this.props.children;
    return <CrashPanel error={this.state.error} stack={this.state.stack} />;
  }
}

function CrashPanel({ error, stack }: { error: Error; stack: string | null }) {
  const { t } = useTranslation();

  return (
    <div className="flex h-screen flex-col items-center justify-center gap-3 bg-bg p-6 text-center">
      <p className="text-heading font-semibold text-text">{t("common.errors.crash")}</p>
      <p className="text-body text-text-dim">{error.message}</p>
      <Button variant="primary" onClick={() => window.location.reload()}>
        {t("common.errors.reload")}
      </Button>
      {stack && (
        <details className="max-w-xl text-left">
          <summary className="cursor-pointer text-small text-text-faint">
            {t("common.errors.details")}
          </summary>
          <pre className="mt-2 max-h-40 overflow-auto rounded-md border border-border bg-inset p-3 font-mono text-caption text-text-dim">
            {stack}
          </pre>
        </details>
      )}
    </div>
  );
}
