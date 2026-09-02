import { STORY_STYLE_PRESETS, STYLE_PRESETS } from "../presets";
import type { StorybookRequestForm } from "../types";
import { PAGE_ASPECT_OPTIONS } from "../../../../utils/pageAspect";

export function RequestStepForm({
  form,
  disabled,
  styleCardsExpanded,
  customStyleOpen,
  mode = "all",
  onChange,
  onToggleStyleCards,
  onToggleCustomStyle,
}: {
  form: StorybookRequestForm;
  disabled: boolean;
  styleCardsExpanded: boolean;
  customStyleOpen: boolean;
  mode?: "all" | "content" | "settings";
  onChange: (patch: Partial<StorybookRequestForm>) => void;
  onToggleStyleCards: () => void;
  onToggleCustomStyle: () => void;
}) {
  const advancedFields = (
    <div className="form-grid">
      <label>页数<input type="number" min={4} max={32} value={form.pageCount} disabled={disabled} onChange={(event) => onChange({ pageCount: event.target.value })} /></label>
      <div className="span-2">
        <span className="field-label">页面比例</span>
        <div className="page-aspect-options compact">
          {PAGE_ASPECT_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              className={`page-aspect-option ${form.pageAspectRatio === option.value ? "active" : ""}`}
              disabled={disabled}
              title={option.hint}
              onClick={() => onChange({ pageAspectRatio: option.value })}
            >
              <span className="page-aspect-shape" style={{ aspectRatio: option.cssRatio }} aria-hidden="true" />
              <strong>{option.label}</strong>
              <small>{option.hint}</small>
            </button>
          ))}
        </div>
      </div>
      <div className="span-2">
        <span className="field-label">画面风格</span>
        <div className="style-preset-grid compact-style-grid">
          {(styleCardsExpanded ? STYLE_PRESETS : recommendedStylePresets()).map((preset) => (
            <button
              key={preset.label}
              type="button"
              className={`style-preset ${form.style === preset.value ? "active" : ""}`}
              disabled={disabled}
              onClick={() => onChange({ style: preset.value })}
            >
              <img src={preset.image} alt={preset.label} loading="lazy" />
              <span className="style-preset-caption">
                <strong>{preset.label}</strong>
                <em>{preset.tag}</em>
              </span>
              {form.style === preset.value && <span className="style-preset-check">✓</span>}
            </button>
          ))}
        </div>
        <div className="inline-actions">
          <button type="button" className="style-preset-toggle" disabled={disabled} onClick={onToggleStyleCards}>
            {styleCardsExpanded ? "收起风格" : `更多风格（${STYLE_PRESETS.length} 种）`}
          </button>
        </div>
      </div>
      <div className="span-2">
        <span className="field-label">故事风格</span>
        <div className="story-style-chips">
          {STORY_STYLE_PRESETS.map((preset) => (
            <button
              key={preset.label}
              type="button"
              className={`story-style-chip ${form.storyStyle === preset.value ? "active" : ""}`}
              disabled={disabled}
              title={preset.value}
              onClick={() => onChange({ storyStyle: preset.value })}
            >
              <strong>{preset.label}</strong>
              <span>{preset.tag}</span>
            </button>
          ))}
        </div>
      </div>
      <label className="span-2">
        故事框架（可选）
        <textarea
          rows={4}
          value={form.storyFramework}
          disabled={disabled}
          placeholder={"可以简单写故事走向；不写也可以，AI 会根据主题自由创作。"}
          onChange={(event) => onChange({ storyFramework: event.target.value })}
        />
      </label>
    </div>
  );

  return (
    <div className="request-step">
      {mode !== "settings" && <>
        <div className="creation-wizard-intro">
          <p className="eyebrow">故事细节</p>
          <h2>补充更多故事信息</h2>
          <p>这些内容会帮助故事更贴近真实场景，不填写也可以继续。</p>
        </div>
        <div className="request-main-fields">
          <label>绘本标题<input value={form.title} disabled={disabled} placeholder="不填则按故事想法自动生成" onChange={(event) => onChange({ title: event.target.value })} /></label>
          <label>主题/目标<input value={form.theme} disabled={disabled} placeholder="不填则按故事想法自动判断" onChange={(event) => onChange({ theme: event.target.value })} /></label>
          <label>
            年龄段
            <select value={form.ageGroup} disabled={disabled} onChange={(event) => onChange({ ageGroup: event.target.value })}>
              <option>3-4 岁</option>
              <option>4-5 岁</option>
              <option>5-6 岁</option>
            </select>
          </label>
          <label>
            使用场景
            <select value={form.useScene} disabled={disabled} onChange={(event) => onChange({ useScene: event.target.value })}>
              <option value="">按故事想法自动判断</option>
              <option>课堂共读</option>
              <option>规则引导</option>
              <option>家园沟通</option>
              <option>入园适应</option>
              <option>睡前故事</option>
              <option>情绪管理</option>
              <option>安全教育</option>
              <option>健康与生活自理</option>
              <option>节日与节气活动</option>
              <option>户外探索</option>
              <option>区域活动延伸</option>
            </select>
          </label>
        </div>
      </>}
      {mode !== "content" && (mode === "settings" ? (
        <div className="section-tools request-advanced-fields request-advanced-fields-plain">
          {advancedFields}
        </div>
      ) : (
        <details className="section-tools request-advanced-fields">
        <summary>
          <span>
            细节设置
            <small>页数、画面风格和故事框架</small>
          </span>
        </summary>
        {advancedFields}
        </details>
      ))}
    </div>
  );
}

function recommendedStylePresets() {
  const preferred = ["水彩", "蜡笔", "卡通", "扁平"];
  const picked = preferred
    .map((keyword) => STYLE_PRESETS.find((preset) => preset.label.includes(keyword) || preset.tag.includes(keyword)))
    .filter((preset): preset is (typeof STYLE_PRESETS)[number] => Boolean(preset));
  return picked.length >= 4 ? picked.slice(0, 4) : STYLE_PRESETS.slice(0, 4);
}
