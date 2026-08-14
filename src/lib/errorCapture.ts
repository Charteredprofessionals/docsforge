import { logBug } from "./ipc";

let installed = false;
const seen = new Set<string>();

/**
 * Installs global handlers that automatically capture uncaught JS errors and
 * unhandled promise rejections into the Bug Book. Failures are best-effort and
 * never interfere with the running app. Repeated identical errors are de-duplicated
 * within a session to avoid flooding the log.
 */
export function installErrorCapture(): void {
  if (installed) return;
  installed = true;

  const report = (
    errorType: string,
    message: string,
    stack: string,
    severity: "critical" | "high" | "medium" = "high"
  ) => {
    const key = `${message}|${stack}`;
    if (seen.has(key)) return;
    seen.add(key);
    logBug({
      errorType,
      message,
      stackTrace: stack,
      severity,
      context: `client:${typeof location !== "undefined" ? location.pathname : "unknown"}`,
      category: "frontend",
      source: "auto",
    }).catch(() => {
      /* best-effort */
    });
  };

  window.onerror = (event, source, lineno, colno, error) => {
    const msg = error?.message ?? String(event);
    const stack = [error?.stack, `at ${source}:${lineno}:${colno}`]
      .filter(Boolean)
      .join("\n");
    report("uncaught_error", msg, stack, "high");
  };

  window.onunhandledrejection = (event) => {
    const reason = (event as PromiseRejectionEvent).reason;
    const message = reason instanceof Error ? reason.message : String(reason);
    const stack = reason instanceof Error ? reason.stack ?? "" : "";
    report("unhandled_rejection", message, stack, "medium");
  };

  // Tauri surface errors (rejected IPC / window) arrive as unhandled rejections,
  // which are already covered above. This guard exists for completeness.
  window.addEventListener("unhandledrejection", () => {
    /* handled by window.onunhandledrejection */
  });
}
