import { ActionButton, Badge, Modal } from "../../../../components/ui";
import type { ExportJob, ShareLink } from "../../../../api/client";
import type { Storybook, StorybookQualityReport, StorybookRole } from "../../../../types/domain";
import {
  exportStatusLabel,
  qualityPageSummary,
  qualityStatusLabel,
  qualityTone,
  shareAccessLabel,
  shareExpiryLabel,
  teacherReviewLabel,
  visibilityLabel,
} from "../helpers";

export function ShareLinksModal({
  workspaceName,
  book,
  exportJobs,
  shareLinks,
  quality,
  firstActionableQualityPage,
  firstRoleNeedingReference,
  effectiveDeliveryBlocker,
  deliveryWarnings,
  reviewDeliveryReminder,
  reviewSaving,
  visibilityValue,
  visibilitySaving,
  shareExpiry,
  shareSaving,
  revokingShareId,
  createdShareUrl,
  qualityDeliveryBlocker,
  onClose,
  onVisibilityChange,
  onSaveVisibility,
  onShareExpiryChange,
  onCreateShare,
  onRevokeShare,
  onCopyShareUrl,
  onSaveTeacherReview,
  onFocusQualityPage,
  onFocusRoleReference,
}: {
  workspaceName: string;
  book: Storybook;
  exportJobs: ExportJob[];
  shareLinks: ShareLink[];
  quality?: StorybookQualityReport;
  firstActionableQualityPage?: StorybookQualityReport["pages"][number];
  firstRoleNeedingReference?: StorybookRole;
  effectiveDeliveryBlocker: string;
  deliveryWarnings: string[];
  reviewDeliveryReminder: string;
  reviewSaving: boolean;
  visibilityValue: Storybook["visibility"];
  visibilitySaving: boolean;
  shareExpiry: "7d" | "30d" | "never";
  shareSaving: boolean;
  revokingShareId: string | null;
  createdShareUrl: string | null;
  qualityDeliveryBlocker: string;
  onClose: () => void;
  onVisibilityChange: (value: Storybook["visibility"]) => void;
  onSaveVisibility: () => void;
  onShareExpiryChange: (value: "7d" | "30d" | "never") => void;
  onCreateShare: () => void;
  onRevokeShare: (link: ShareLink) => void;
  onCopyShareUrl: (link: ShareLink) => void;
  onSaveTeacherReview: (status: "pending" | "confirmed") => void;
  onFocusQualityPage: (page: StorybookQualityReport["pages"][number]) => void;
  onFocusRoleReference: (role: StorybookRole) => void;
}) {
  return (
    <Modal title="管理分享链接" onClose={onClose}>
      <section className="share-section">
        <p className="share-meta">分享范围：获得链接的人可查看当前绘本版本 · 当前空间：<strong>{workspaceName}</strong></p>
        <p className="share-meta">
          可见性 <strong>{visibilityLabel(book.visibility)}</strong>
          <span className="share-meta-sep">·</span>导出 <strong>{exportJobs.length ? exportStatusLabel(exportJobs[0].status) : "暂无记录"}</strong>
          <span className="share-meta-sep">·</span>分享链接 <strong>{shareLinks.length ? `${shareLinks.length} 个有效链接` : "未创建"}</strong>
          <span className="share-meta-sep">·</span>复核 <strong>{teacherReviewLabel(book.teacherReviewStatus)}</strong>
        </p>
        <div className="delivery-status-main modal-delivery-status">
          <div>
            <p className="eyebrow">分享前检查</p>
            <h3>{effectiveDeliveryBlocker ? "先处理阻断项，再分享" : book.teacherReviewStatus === "confirmed" ? "已复核，可以分享" : "建议老师复核后分享"}</h3>
            <p>{effectiveDeliveryBlocker || deliveryWarnings[0] || reviewDeliveryReminder || "页面、角色和插图检查已通过，可以创建分享链接。"}</p>
          </div>
          {quality && (
            <div className="delivery-status-actions">
              <Badge tone={qualityTone(quality.status)}>{qualityStatusLabel(quality.status)}</Badge>
              <button
                className={book.teacherReviewStatus === "confirmed" ? "button secondary" : "button primary"}
                type="button"
                disabled={reviewSaving || (book.teacherReviewStatus !== "confirmed" && quality.status === "blocked")}
                title={book.teacherReviewStatus !== "confirmed" && quality.status === "blocked" ? "请先修正生成质量阻断项" : undefined}
                onClick={() => onSaveTeacherReview(book.teacherReviewStatus === "confirmed" ? "pending" : "confirmed")}
              >
                {reviewSaving ? "保存中..." : book.teacherReviewStatus === "confirmed" ? "重新设为待复核" : quality.status === "blocked" ? "先修正阻断项" : "老师已复核"}
              </button>
            </div>
          )}
        </div>
        {quality && (firstActionableQualityPage || firstRoleNeedingReference) && (
          <div className="delivery-next-step">
            <div>
              <strong>{quality.status === "blocked" ? "需要处理" : "建议查看"}</strong>
              <span>
                {firstActionableQualityPage
                  ? `第 ${firstActionableQualityPage.pageNumber} 页：${firstActionableQualityPage.issues[0] || firstActionableQualityPage.suggestions[0] || "请核对分页内容。"}`
                  : `${firstRoleNeedingReference?.name} 还没有可用参考图。`}
              </span>
            </div>
            <div className="inline-actions">
              {firstActionableQualityPage && (
                <button className="button secondary" type="button" onClick={() => onFocusQualityPage(firstActionableQualityPage)}>
                  定位问题页
                </button>
              )}
              {firstRoleNeedingReference && (
                <button className="button secondary" type="button" onClick={() => onFocusRoleReference(firstRoleNeedingReference)}>
                  定位角色参考图
                </button>
              )}
            </div>
          </div>
        )}
        <p className="share-meta privacy-note">分享前请确认不包含未授权儿童信息或家庭隐私。</p>
        {quality && (
          <details className="quality-details compact">
            <summary>查看作品检查详情</summary>
            <div className="quality-check-grid">
              {quality.checks.map((check) => (
                <div className="quality-check-item" key={check.key}>
                  <Badge tone={qualityTone(check.status)}>{qualityStatusLabel(check.status)}</Badge>
                  <strong>{check.label}</strong>
                  <span>{check.message}</span>
                </div>
              ))}
            </div>
            <div className="quality-page-list">
              {quality.pages.map((page) => (
                <button className="quality-page-row" type="button" key={page.pageId} onClick={() => onFocusQualityPage(page)}>
                  <div>
                    <strong>第 {page.pageNumber} 页</strong>
                    <span>{qualityPageSummary(page)}</span>
                    {(page.issues.length > 0 || page.suggestions.length > 0) && (
                      <div className="quality-page-notes">
                        {page.issues.map((issue) => (
                          <small className="quality-page-note issue" key={`issue-${issue}`}>问题：{issue}</small>
                        ))}
                        {page.suggestions.map((suggestion) => (
                          <small className="quality-page-note suggestion" key={`suggestion-${suggestion}`}>建议：{suggestion}</small>
                        ))}
                      </div>
                    )}
                  </div>
                  <Badge tone={qualityTone(page.status)}>{qualityStatusLabel(page.status)}</Badge>
                </button>
              ))}
            </div>
          </details>
        )}
      </section>

      <section className="share-section">
        <h3 className="share-section-title">分享设置</h3>
        <div className="form-grid">
          <label>
            整本绘本可见范围
            <select value={visibilityValue} onChange={(event) => onVisibilityChange(event.target.value as Storybook["visibility"])}>
              <option value="private">仅当前空间私有</option>
              <option value="workspace">园所/空间内共享</option>
            </select>
          </label>
          <button className="button secondary" type="button" disabled={visibilitySaving || visibilityValue === book.visibility} onClick={onSaveVisibility}>
            {visibilitySaving ? "保存中..." : visibilityValue === book.visibility ? "可见范围已保存" : "保存可见范围"}
          </button>
        </div>
      </section>

      <section className="share-section">
        <h3 className="share-section-title">分享链接</h3>
        {shareLinks.length ? (
          <div className="share-link-list">
            {shareLinks.map((link, index) => (
              <div className="share-link-row" key={link.id}>
                <div>
                  <strong>分享链接 {index + 1}</strong>
                  <span>{shareExpiryLabel(link.expiresAt)}</span>
                  <span>{shareAccessLabel(link)}</span>
                </div>
                <div className="inline-actions">
                  <a className="button secondary" href={link.url} target="_blank" rel="noreferrer">打开</a>
                  <button className="button secondary" type="button" onClick={() => onCopyShareUrl(link)}>复制链接</button>
                  <button className="button secondary" type="button" disabled={shareSaving} onClick={() => onRevokeShare(link)}>
                    {revokingShareId === link.id ? "撤回中..." : "撤回"}
                  </button>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className="share-meta">还没有有效分享链接。</p>
        )}
        <div className="form-grid">
          <label>
            链接有效期
            <select value={shareExpiry} onChange={(event) => onShareExpiryChange(event.target.value as "7d" | "30d" | "never")}>
              <option value="7d">7 天有效</option>
              <option value="30d">30 天有效</option>
              <option value="never">不过期</option>
            </select>
          </label>
        </div>
        <div className="modal-actions share-actions">
          <button className="button secondary" type="button" onClick={onClose}>关闭</button>
          {createdShareUrl && <a className="button secondary" href={createdShareUrl} target="_blank" rel="noreferrer">打开最新分享页</a>}
          <ActionButton className="button primary" disabled={shareSaving || Boolean(qualityDeliveryBlocker)} disabledHint={qualityDeliveryBlocker || (shareSaving ? "处理中，请稍候" : undefined)} onClick={onCreateShare}>
            {shareSaving ? "处理中..." : "创建新的分享链接"}
          </ActionButton>
        </div>
      </section>
    </Modal>
  );
}
