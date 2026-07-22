import type { ReactNode } from "react";

export function Badge({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "good" | "warn" | "danger" | "info" }) {
  return <span className={`badge badge-${tone}`}>{children}</span>;
}

export function Card({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <section className={`card ${className}`}>{children}</section>;
}

export function EmptyState({ title, copy, action }: { title: string; copy: string; action?: ReactNode }) {
  return (
    <div className="empty-state">
      <strong>{title}</strong>
      <p>{copy}</p>
      {action}
    </div>
  );
}

export function Notice({
  title,
  copy,
  action,
  tone = "good",
}: {
  title: string;
  copy: string;
  action?: ReactNode;
  tone?: "good" | "warn" | "danger" | "info";
}) {
  return (
    <div className={`notice notice-${tone}`} role="status">
      <div>
        <strong>{title}</strong>
        <span>{copy}</span>
      </div>
      {action}
    </div>
  );
}

export function PageHeader({
  eyebrow,
  title,
  copy,
  actions,
}: {
  eyebrow?: string;
  title: string;
  copy: string;
  actions?: ReactNode;
}) {
  return (
    <header className="page-header">
      <div>
        {eyebrow && <p className="eyebrow">{eyebrow}</p>}
        <h1>{title}</h1>
        <p>{copy}</p>
      </div>
      {actions && <div className="page-actions">{actions}</div>}
    </header>
  );
}

export function ProgressSteps({ steps, active }: { steps: string[]; active: number }) {
  return (
    <ol className="steps">
      {steps.map((step, index) => (
        <li key={step} className={index === active ? "active" : index < active ? "done" : ""}>
          <span>{index + 1}</span>
          {step}
        </li>
      ))}
    </ol>
  );
}

export function WizardSideNav({
  title,
  copy,
  steps,
  active,
  onSelect,
}: {
  title: string;
  copy: string;
  steps: string[];
  active: number;
  onSelect: (step: number) => void;
}) {
  return (
    <aside className="wizard-side-nav" aria-label={title}>
      <div className="wizard-side-head">
        <p className="eyebrow">流程导航</p>
        <h2>{title}</h2>
        <p>{copy}</p>
      </div>
      <ol>
        {steps.map((step, index) => (
          <li key={step}>
            <button
              type="button"
              className={index === active ? "active" : index < active ? "done" : ""}
              onClick={() => onSelect(index)}
              aria-current={index === active ? "step" : undefined}
            >
              <span>{index + 1}</span>
              <strong>{step}</strong>
            </button>
          </li>
        ))}
      </ol>
    </aside>
  );
}

export function Modal({
  title,
  children,
  onClose,
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
}) {
  return (
    <div className="modal-backdrop" role="dialog" aria-modal="true" aria-label={title}>
      <div className="modal">
        <div className="modal-head">
          <h2>{title}</h2>
          <button className="icon-button" type="button" onClick={onClose} aria-label="关闭">
            x
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}

export function statusTone(status: string): "neutral" | "good" | "warn" | "danger" | "info" {
  if (["exportable", "listed", "approved", "active", "ready", "succeeded"].includes(status)) return "good";
  if (["submitted", "plan_pending", "roles_pending", "image_pending", "needs_regeneration", "generating", "queued", "running"].includes(status)) return "warn";
  if (["rejected", "expired", "revoked", "failed"].includes(status)) return "danger";
  if (["editing", "draft"].includes(status)) return "info";
  return "neutral";
}
