import { STORY_STYLE_PRESETS, STYLE_PRESETS } from "../presets";
import type { StorybookRequestForm } from "../types";
import { PAGE_ASPECT_OPTIONS } from "../../../../utils/pageAspect";

export function RequestStepForm({
  form,
  disabled,
  styleCardsExpanded,
  onChange,
  onToggleStyleCards,
}: {
  form: StorybookRequestForm;
  disabled: boolean;
  styleCardsExpanded: boolean;
  onChange: (patch: Partial<StorybookRequestForm>) => void;
  onToggleStyleCards: () => void;
}) {
  return (
    <div className="form-grid">
      <label>绘本标题<input value={form.title} disabled={disabled} onChange={(event) => onChange({ title: event.target.value })} /></label>
      <label>绘本主题<input value={form.theme} disabled={disabled} onChange={(event) => onChange({ theme: event.target.value })} /></label>
      <label>
        年龄段
        <select value={form.ageGroup} disabled={disabled} onChange={(event) => onChange({ ageGroup: event.target.value })}>
          <option>3-4 岁</option>
          <option>4-5 岁</option>
          <option>5-6 岁</option>
        </select>
      </label>
      <label>页数<input type="number" value={form.pageCount} disabled={disabled} onChange={(event) => onChange({ pageCount: event.target.value })} /></label>
      <label>
        使用场景
        <select value={form.useScene} disabled={disabled} onChange={(event) => onChange({ useScene: event.target.value })}>
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
      <div className="span-2">
        <span className="field-label">页面比例</span>
        <div className="page-aspect-options">
          {PAGE_ASPECT_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              className={`page-aspect-option ${form.pageAspectRatio === option.value ? "active" : ""}`}
              disabled={disabled}
              onClick={() => onChange({ pageAspectRatio: option.value })}
            >
              <span className="page-aspect-shape" style={{ aspectRatio: option.cssRatio }} aria-hidden="true" />
              <strong>{option.label}</strong>
              <small>{option.hint}</small>
            </button>
          ))}
        </div>
        <p className="form-hint">页面比例会同时影响插图生成尺寸、详情预览和 PDF 导出页面。</p>
      </div>
      <div className="span-2">
        <span className="field-label">画面风格</span>
        <div className="style-preset-grid">
          {(styleCardsExpanded ? STYLE_PRESETS : STYLE_PRESETS.slice(0, 6)).map((preset) => (
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
        <button type="button" className="style-preset-toggle" disabled={disabled} onClick={onToggleStyleCards}>
          {styleCardsExpanded ? "收起风格 ▲" : `展开更多风格（共 ${STYLE_PRESETS.length} 种）▼`}
        </button>
        <textarea
          rows={2}
          value={form.style}
          disabled={disabled}
          placeholder="选择上方预设风格，或在这里直接描述想要的风格"
          onChange={(event) => onChange({ style: event.target.value })}
        />
        <p className="form-hint">选中预设后可直接在文本框里微调，生成时会按这里的描述执行。</p>
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
        <textarea
          rows={2}
          value={form.storyStyle}
          disabled={disabled}
          placeholder="选择上方预设故事风格，或在这里直接描述想要的情节基调"
          onChange={(event) => onChange({ storyStyle: event.target.value })}
        />
        <p className="form-hint">故事风格决定情节基调，画面风格决定怎么画，两者可自由组合；文本框留空则由 AI 自由发挥。</p>
      </div>
      <label className="span-2">
        故事框架（可选）
        <textarea
          rows={4}
          value={form.storyFramework}
          disabled={disabled}
          placeholder={"可以简单写几句故事的走向，比如：\n开头：小猫第一次上幼儿园，紧紧抓着妈妈的手。\n经过：它不敢加入游戏，后来在小兔的邀请下一起搭积木。\n结尾：放学时小猫已经交到了两个好朋友。\n不写也可以，AI 会根据主题自由创作。"}
          onChange={(event) => onChange({ storyFramework: event.target.value })}
        />
        <p className="form-hint">填写后，AI 会严格按你的框架展开分页；留空则由 AI 自由创作。</p>
      </label>
    </div>
  );
}
