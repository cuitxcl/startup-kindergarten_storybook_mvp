export function WizardTopNav({
  steps,
  active,
  maxUnlockedStep,
  disabled,
  status,
  onSelect,
}: {
  steps: string[];
  active: number;
  maxUnlockedStep: number;
  disabled: boolean;
  status?: string;
  onSelect: (step: number) => void;
}) {
  return (
    <nav className="wizard-top-nav" aria-label="普通绘本流程">
      <ol>
        {steps.map((step, index) => {
          const locked = index > maxUnlockedStep;
          const done = index < active;
          const current = index === active;
          return (
            <li key={step}>
              <button
                type="button"
                className={locked ? "locked" : current ? "active" : done ? "done" : ""}
                disabled={locked}
                aria-disabled={disabled || locked || undefined}
                title={locked ? "请先完成前一步" : disabled ? "生成进行中，请稍候" : undefined}
                onClick={disabled ? undefined : () => onSelect(index)}
                aria-current={current ? "step" : undefined}
              >
                <span>{done ? "✓" : index + 1}</span>
                <strong>{step}</strong>
              </button>
            </li>
          );
        })}
      </ol>
      {status && <p>{status}</p>}
    </nav>
  );
}
