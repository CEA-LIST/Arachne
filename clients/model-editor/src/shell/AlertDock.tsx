/**
 * Failure surface, docked rather than banner-width.
 *
 * The old full-width red bar reflowed every panel below it the moment anything
 * failed. This floats over the content (position: absolute), so a failure is
 * impossible to miss and impossible to confuse with a layout change. It
 * auto-dismisses after 8 s; the console entry it points at is permanent.
 */

import { useEffect } from 'react';
import { CircleAlert, X } from '../ui/icons';
import { ICON } from '../ui/iconProps';

/** How long a dismissable alert stays up before it retires itself. */
export const ALERT_TIMEOUT_MS = 8000;

interface AlertDockProps {
  message: string | null;
  onDismiss: () => void;
  onShowDetails: () => void;
}

export function AlertDock({ message, onDismiss, onShowDetails }: AlertDockProps) {
  useEffect(() => {
    if (message === null) return;
    const timer = setTimeout(onDismiss, ALERT_TIMEOUT_MS);
    return () => clearTimeout(timer);
  }, [message, onDismiss]);

  if (message === null) return null;

  return (
    <div className="me-alertdock me-noprint" role="alert">
      <CircleAlert {...ICON} className="me-alertdock__icon" aria-hidden="true" />
      <span className="me-alertdock__message">{message}</span>
      <button type="button" className="me-btn me-btn--sm" onClick={onShowDetails}>
        Details
      </button>
      <button type="button" className="me-iconbtn" onClick={onDismiss} aria-label="Dismiss alert">
        <X {...ICON} size={14} aria-hidden="true" />
      </button>
    </div>
  );
}
