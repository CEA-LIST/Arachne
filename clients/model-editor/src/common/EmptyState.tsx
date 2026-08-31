/**
 * The shared empty-state card.
 *
 * Every blank surface in this app instructs: what the state is, why it is that
 * way, and the one action that leaves it. A study participant meeting a panel
 * for the first time should never see a bare page.
 */

import type { ReactNode } from 'react';
import { type LucideIcon } from '../ui/icons';
import { ICON_LG } from '../ui/iconProps';

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  body: ReactNode;
  children?: ReactNode;
  tone?: 'neutral' | 'warn';
}

export function EmptyState({ icon: Icon, title, body, children, tone = 'neutral' }: EmptyStateProps) {
  return (
    <div className="me-empty">
      <div className={`me-empty__card me-empty__card--${tone}`}>
        <Icon {...ICON_LG} className="me-empty__icon" aria-hidden="true" />
        <h2 className="me-empty__title">{title}</h2>
        <div className="me-empty__body">{body}</div>
        {children !== undefined && <div className="me-empty__actions">{children}</div>}
      </div>
    </div>
  );
}
