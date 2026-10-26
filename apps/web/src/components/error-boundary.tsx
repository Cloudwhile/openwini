import { Component, type ReactNode } from "react"
import { ErrorState } from "@/components/page-states"
import { t } from "@/i18n"

export class ErrorBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false }

  static getDerivedStateFromError() {
    return { failed: true }
  }

  render() {
    if (this.state.failed) {
      return (
        <main className="mx-auto max-w-lg px-4 py-16">
          <ErrorState message={t.errors.network} onRetry={() => window.location.reload()} />
        </main>
      )
    }
    return this.props.children
  }
}
