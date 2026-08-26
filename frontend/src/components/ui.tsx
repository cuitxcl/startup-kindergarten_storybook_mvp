import { ChevronLeft, ChevronRight, MoreHorizontal } from "lucide-react";
import { useEffect, useId, useRef, useState, type ComponentPropsWithoutRef, type ReactNode } from "react";

export function Badge({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "good" | "warn" | "danger" | "info" }) {
  return <span className={`badge badge-${tone}`}>{children}</span>;
}

export function Card({ children, className = "", ...props }: { children: ReactNode; className?: string } & ComponentPropsWithoutRef<"section">) {
  return <section {...props} className={`card ${className}`}>{children}</section>;
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

/** 成功类轻提示：右上角浮出，2.5s 后自动消失。阻断/需处理的信息仍用 Notice。 */
export function Toast({ title, copy, onClose }: { title: string; copy?: string; onClose: () => void }) {
  useEffect(() => {
    const timer = window.setTimeout(onClose, 2500);
    return () => window.clearTimeout(timer);
  }, [onClose]);
  return (
    <div className="toast" role="status">
      <strong>{title}</strong>
      {copy && <span>{copy}</span>}
    </div>
  );
}

/** 骨架占位块：加载期间保持版面稳定，避免内容跳动。 */
export function SkeletonBlock({ className = "", lines = 0 }: { className?: string; lines?: number }) {
  if (lines > 0) {
    return (
      <div className={`skeleton-lines ${className}`} aria-hidden="true">
        {Array.from({ length: lines }, (_, index) => (
          <span key={index} className="skeleton skeleton-line" style={{ width: `${88 - index * 14}%` }} />
        ))}
      </div>
    );
  }
  return <div className={`skeleton ${className}`} aria-hidden="true" />;
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

export function ProgressSteps({
  steps,
  active,
  onStepClick,
  maxUnlockedStep = active,
}: {
  steps: string[];
  active: number;
  onStepClick?: (index: number) => void;
  maxUnlockedStep?: number;
}) {
  return (
    <ol className="steps" style={{ gridTemplateColumns: `repeat(${steps.length}, minmax(0, 1fr))` }}>
      {steps.map((step, index) => {
        const canNavigate = Boolean(onStepClick) && index <= maxUnlockedStep && index !== active;
        return (
          <li key={step} className={index === active ? "active" : index < active ? "done" : ""}>
            {canNavigate ? (
              <button className="step-button" type="button" onClick={() => onStepClick?.(index)}>
                <span>{index + 1}</span>
                {step}
              </button>
            ) : <><span>{index + 1}</span>{step}</>}
          </li>
        );
      })}
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

export function OverflowMenu({
  label = "更多操作",
  children,
}: {
  label?: string;
  children: (close: () => void) => ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const menuId = useId();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const close = () => {
    setOpen(false);
    window.requestAnimationFrame(() => triggerRef.current?.focus());
  };

  useEffect(() => {
    if (!open) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") close();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open]);

  return (
    <div className="more-menu">
      <button ref={triggerRef} className="icon-button overflow-menu-trigger" type="button" aria-label={label} title={label} aria-expanded={open} aria-controls={open ? menuId : undefined} onClick={() => setOpen((current) => !current)}>
        <MoreHorizontal size={18} aria-hidden="true" />
      </button>
      {open && <><button className="menu-overlay" type="button" tabIndex={-1} aria-hidden="true" onClick={close} /><div id={menuId} className="more-menu-pop">{children(close)}</div></>}
    </div>
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
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const dialog = dialogRef.current;
    const focusableSelector = [
      "a[href]",
      "button:not([disabled])",
      "textarea:not([disabled])",
      "input:not([disabled])",
      "select:not([disabled])",
      "[tabindex]:not([tabindex='-1'])",
    ].join(",");

    function focusFirstControl() {
      const focusable = dialog ? Array.from(dialog.querySelectorAll<HTMLElement>(focusableSelector)) : [];
      (focusable[0] || dialog)?.focus();
    }

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
        return;
      }

      if (event.key !== "Tab" || !dialog) return;
      const focusable = Array.from(dialog.querySelectorAll<HTMLElement>(focusableSelector)).filter(
        (element) => element.offsetParent !== null,
      );
      if (focusable.length === 0) {
        event.preventDefault();
        dialog.focus();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }

    focusFirstControl();
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      previousFocus?.focus();
    };
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
      <div className={`modal ${className}`} ref={dialogRef} tabIndex={-1}>
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
  primaryAction,
  previousAction,
  nextAction,
  positionLabel,
}: {
  src: string;
  alt: string;
  onClose: () => void;
  primaryAction?: { label: string; onClick: () => void; disabled?: boolean };
  previousAction?: () => void;
  nextAction?: () => void;
  positionLabel?: string;
}) {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
      if (event.key === "ArrowLeft" && previousAction) {
        event.preventDefault();
        previousAction();
      }
      if (event.key === "ArrowRight" && nextAction) {
        event.preventDefault();
        nextAction();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [nextAction, onClose, previousAction]);

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
        {previousAction && (
          <button className="icon-button image-lightbox-nav image-lightbox-nav-prev" type="button" onClick={previousAction} aria-label="查看上一张候选图" title="上一张">
            <ChevronLeft size={24} aria-hidden="true" />
          </button>
        )}
        <img src={src} alt={alt} />
        {nextAction && (
          <button className="icon-button image-lightbox-nav image-lightbox-nav-next" type="button" onClick={nextAction} aria-label="查看下一张候选图" title="下一张">
            <ChevronRight size={24} aria-hidden="true" />
          </button>
        )}
        {alt ? <figcaption>{alt}</figcaption> : null}
        {positionLabel ? <span className="image-lightbox-position">{positionLabel}</span> : null}
        {primaryAction && (
          <div className="image-lightbox-actions">
            <button className="button primary" type="button" disabled={primaryAction.disabled} onClick={primaryAction.onClick}>
              {primaryAction.label}
            </button>
          </div>
        )}
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
