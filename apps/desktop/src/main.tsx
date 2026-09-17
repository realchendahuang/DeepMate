import React from "react";
import ReactDOM from "react-dom/client";
import { toast } from "sonner";
import App from "./App";
import { ErrorBoundary } from "./app/error-boundary";
import { mapError } from "./shared/lib/errors";
import i18n from "./i18n";
import "./styles.css";
import "./i18n";

// The error boundary only covers render failures. Command calls that are
// fired without a handler (a click that does not await, a background
// refresh) used to fail into the console where nobody would ever see them;
// surfacing them as a toast keeps every failure visible to the user.
window.addEventListener("unhandledrejection", (event) => {
  const reason = event.reason;
  // An already-translated message means some layer handled the wording;
  // still show it rather than dropping the failure.
  const key = `common.errors.${mapError(reason)}`;
  toast.error(i18n.t(key));
  console.error("unhandled rejection:", reason);
});

window.addEventListener("error", (event) => {
  // Resource-loading errors also land here and carry no useful message.
  if (!event.message) return;
  console.error("uncaught error:", event.error ?? event.message);
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>,
);
