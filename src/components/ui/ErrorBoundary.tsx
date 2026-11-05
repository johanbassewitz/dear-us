import { Component, type ReactNode } from "react";
import { EmptyState } from "./EmptyState";
import { Button } from "./Button";

/** Keeps one broken screen from blanking the whole app. Memories are never affected. */
export class ErrorBoundary extends Component<{ children: ReactNode; resetKey?: unknown }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error) {
    console.error("Dear Us screen error:", error);
  }

  componentDidUpdate(prev: { resetKey?: unknown }) {
    if (prev.resetKey !== this.props.resetKey && this.state.error) this.setState({ error: null });
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="card">
        <EmptyState
          title="This screen hit a snag"
          action={
            <Button variant="secondary" onClick={() => this.setState({ error: null })}>
              Try again
            </Button>
          }
        >
          Your memories are safe on this PC. Try again, or open another page. Detail: {this.state.error.message}
        </EmptyState>
      </div>
    );
  }
}
