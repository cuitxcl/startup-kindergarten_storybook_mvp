import { useEffect, type ReactNode } from "react";

export function Badge({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "good" | "warn" | "danger" | "info" }) {
  return <span className={`badge badge-${tone}`}>{children}</span>;
}

export function Card({ children, className = "", id }: { children: ReactNode; className?: string; id?: string }) {
  return <section id={id} className={`card ${className}`}>{children}</section>;
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
  actionClassName,
  className = "",
}: {
  eyebrow?: string;
  title: string;
  copy: string;
  actions?: ReactNode;
  actionClassName?: string;
  className?: string;
}) {
  return (
    <header className={`page-header${className ? ` ${className}` : ""}`}>
      <div>
        {eyebrow && <p className="eyebrow">{eyebrow}</p>}
        <h1>{title}</h1>
        <p>{copy}</p>
      </div>
      {actions && <div className={`page-actions${actionClassName ? ` ${actionClassName}` : ""}`}>{actions}</div>}
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
  maxUnlockedStep = active,
}: {
  title: string;
  copy: string;
  steps: string[];
  active: number;
  onSelect: (step: number) => void;
  maxUnlockedStep?: number;
}) {
  return (
    <aside className="wizard-side-nav" aria-label={title}>
      <div className="wizard-side-head">
        <p className="eyebrow">流程导航</p>
        <h2>{title}</h2>
        <p>{copy}</p>
      </div>
      <ol>
        {steps.map((step, index) => {
          const locked = index > maxUnlockedStep;
          return (
          <li key={step}>
            <button
              type="button"
              className={locked ? "locked" : index === active ? "active" : index < active ? "done" : ""}
              disabled={locked}
              title={locked ? "请先完成前一步" : undefined}
              onClick={() => onSelect(index)}
              aria-current={index === active ? "step" : undefined}
            >
              <span>{index + 1}</span>
              <strong>{step}</strong>
            </button>
          </li>
          );
        })}
      </ol>
    </aside>
  );
}

/**
 * 带禁用提示的按钮：不用原生 disabled 属性（禁用时 title 提示不会显示），
 * 改用 aria-disabled + 点击拦截，让"为什么不能用"的提示在悬停时可见。
 */
export function ActionButton({
  className = "button secondary",
  type = "button",
  disabled = false,
  disabledHint,
  onClick,
  children,
}: {
  className?: string;
  type?: "button" | "submit";
  disabled?: boolean;
  disabledHint?: string;
  onClick?: () => void;
  children: ReactNode;
}) {
  return (
    <button
      className={disabled ? `${className} is-disabled` : className}
      type={type}
      aria-disabled={disabled || undefined}
      title={disabled && disabledHint ? disabledHint : undefined}
      onClick={disabled ? undefined : onClick}
    >
      {children}
    </button>
  );
}

export function Modal({
  title,
  children,
  onClose,
  className = "",
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  className?: string;
}) {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className="modal-backdrop"
      role="dialog"
      aria-modal="true"
      aria-label={title}
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className={`modal ${className}`}>
        <div className="modal-head">
          <h2>{title}</h2>
          <button className="icon-button" type="button" onClick={onClose} aria-label="关闭">
            ×
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}

export function ImageLightbox({
  src,
  alt,
  onClose,
}: {
  src: string;
  alt: string;
  onClose: () => void;
}) {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className="modal-backdrop image-lightbox-backdrop"
      role="dialog"
      aria-modal="true"
      aria-label={alt || "图片放大预览"}
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <figure className="image-lightbox">
        <button className="icon-button image-lightbox-close" type="button" onClick={onClose} aria-label="关闭放大预览">
          ×
        </button>
        <img src={src} alt={alt} onClick={onClose} />
        {alt ? <figcaption>{alt}</figcaption> : null}
      </figure>
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
