// From https://react.dev/reference/react/Component
//
import * as React from "react";

export class ErrorBoundary extends React.Component<
  React.PropsWithChildren<{ fallback: React.JSX.Element }>,
  { hasError: boolean }
> {
  state = { hasError: false };
  static getDerivedStateFromError() {
    // Update state so the next render will show the fallback UI.
    return { hasError: true };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo): void {
    console.log(
      `Caught error: ${error}\n${errorInfo.componentStack},\n\nOwner stack: ${React.captureOwnerStack()}`,
    );
  }

  render() {
    if (this.state.hasError) {
      // You can render any custom fallback UI
      return this.props.fallback;
    }

    return this.props.children;
  }
}
