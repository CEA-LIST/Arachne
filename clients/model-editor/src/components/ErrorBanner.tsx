interface ErrorBannerProps {
  message: string | null;
  onDismiss: () => void;
}

/** Every failed or refused operation is VISIBLE here — never console-only. */
export function ErrorBanner({ message, onDismiss }: ErrorBannerProps) {
  if (message === null) return null;
  return (
    <div className="error-banner" role="alert">
      <span>{message}</span>
      <button type="button" onClick={onDismiss} aria-label="dismiss">
        ×
      </button>
    </div>
  );
}
