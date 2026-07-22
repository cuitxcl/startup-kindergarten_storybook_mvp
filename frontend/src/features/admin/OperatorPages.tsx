import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  approveOperatorSubmission,
  getOperatorGenerationProvider,
  getOperatorReadiness,
  getOperatorStorage,
  listOperatorGenerationCostsPage,
  listMarketplaceTemplatesPage,
  listOperatorSubmissionsPage,
  rejectOperatorSubmission,
  shouldUseApi,
  type GenerationCostReport,
  type GenerationProviderStatus,
  type OperatorReadiness,
  type PaginationMeta,
  type StorageStatus,
  updateOperatorMarketplaceTemplate,
} from "../../api/client";
import { Badge, Card, EmptyState, Modal, Notice, PageHeader, statusTone } from "../../components/ui";
import { marketplaceTemplates, submissions } from "../../data/mock";
import type { MarketplaceSubmission, MarketplaceTemplate } from "../../types/domain";
import { submissionStatusLabel } from "../../utils/labels";

const OPERATOR_PAGE_SIZE = 12;

export function OperatorMarketplacePage() {
  const [notice, setNotice] = useState<string | null>(null);
  const [templates, setTemplates] = useState<MarketplaceTemplate[]>(shouldUseApi ? [] : marketplaceTemplates);
  const [editingTemplate, setEditingTemplate] = useState<MarketplaceTemplate | null>(null);
  const [templateForm, setTemplateForm] = useState({
    title: "",
    summary: "",
    ageGroup: "4-5 岁",
    useScene: "",
    supportsCustomization: true,
    tags: "",
  });
  const [templateOffset, setTemplateOffset] = useState(0);
  const [templateReloadTick, setTemplateReloadTick] = useState(0);
  const [templateMeta, setTemplateMeta] = useState<PaginationMeta | null>(null);
  const [loading, setLoading] = useState(shouldUseApi);
  const [savingTemplate, setSavingTemplate] = useState(false);
  const [error, setError] = useState("");
  const [provider, setProvider] = useState<GenerationProviderStatus | null>(null);
  const [providerLoading, setProviderLoading] = useState(shouldUseApi);
  const [readiness, setReadiness] = useState<OperatorReadiness | null>(null);
  const [readinessLoading, setReadinessLoading] = useState(shouldUseApi);
  const [readinessError, setReadinessError] = useState("");
  const [storage, setStorage] = useState<StorageStatus | null>(
    shouldUseApi
      ? null
      : {
        backend: "local",
        exportsDir: "tmp/exports",
        generatedImagesDir: "tmp/generated-images",
        exportMaxBytes: 52_428_800,
        generatedImageMaxBytes: 15_728_640,
        filenameValidation: true,
        sizeLimitEnabled: true,
        downloadStrategy: "authenticated_api",
        publicDirectAccess: false,
      },
  );
  const [storageLoading, setStorageLoading] = useState(shouldUseApi);
  const [storageError, setStorageError] = useState("");
  const [costReport, setCostReport] = useState<GenerationCostReport | null>(null);
  const [costMeta, setCostMeta] = useState<PaginationMeta | null>(null);
  const [costLoading, setCostLoading] = useState(shouldUseApi);
  const [costError, setCostError] = useState("");
  const [costFilters, setCostFilters] = useState({ provider: "", jobType: "", status: "" });
  const initialLoading = loading && (!shouldUseApi || templates.length === 0);

  const refreshTemplates = () => {
    setTemplateOffset(0);
    setTemplateReloadTick((value) => value + 1);
  };

  const openTemplateEditor = (template: MarketplaceTemplate) => {
    setEditingTemplate(template);
    setTemplateForm({
      title: template.title,
      summary: template.summary,
      ageGroup: template.ageGroup,
      useScene: template.useScene,
      supportsCustomization: template.supportsCustomization,
      tags: template.tags.join("、"),
    });
    setNotice(null);
  };

  async function saveTemplateEdit() {
    if (!editingTemplate) return;
    const title = templateForm.title.trim();
    const summary = templateForm.summary.trim();
    const ageGroup = templateForm.ageGroup.trim();
    const useScene = templateForm.useScene.trim();
    const tags = templateForm.tags
      .split(/[、,，]/)
      .map((tag) => tag.trim())
      .filter(Boolean);
    if (!title || !summary || !ageGroup || !useScene) {
      setNotice("模板标题、摘要、年龄段和场景不能为空。");
      return;
    }

    setSavingTemplate(true);
    try {
      const updated = shouldUseApi
        ? await updateOperatorMarketplaceTemplate(editingTemplate.id, {
          title,
          summary,
          ageGroup,
          useScene,
          supportsCustomization: templateForm.supportsCustomization,
          tags,
        })
        : {
          ...editingTemplate,
          title,
          summary,
          ageGroup,
          useScene,
          supportsCustomization: templateForm.supportsCustomization,
          tags,
        };
      setTemplates((items) => items.map((item) => item.id === updated.id ? updated : item));
      setEditingTemplate(null);
      setNotice(`《${updated.title}》的市场展示信息已保存。`);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "模板保存失败，请稍后重试。");
    } finally {
      setSavingTemplate(false);
    }
  }

  useEffect(() => {
    if (!shouldUseApi) return;
    let mounted = true;
    setLoading(true);
    if (templateOffset === 0) {
      setTemplates([]);
      setTemplateMeta(null);
    }
    setError("");
    listMarketplaceTemplatesPage({ limit: OPERATOR_PAGE_SIZE, offset: templateOffset })
      .then((page) => {
        if (!mounted) return;
        setTemplates((items) => (
          templateOffset === 0
            ? page.data
            : [...items, ...page.data.filter((template) => !items.some((item) => item.id === template.id))]
        ));
        setTemplateMeta(page.meta);
        setError("");
      })
      .catch((err: Error) => {
        if (!mounted) return;
        if (templateOffset === 0) {
          setTemplates([]);
          setTemplateMeta(null);
        }
        setError(err.message);
      })
      .finally(() => {
        if (mounted) setLoading(false);
      });
    return () => {
      mounted = false;
    };
  }, [templateOffset, templateReloadTick]);

  useEffect(() => {
    if (!shouldUseApi) return;
    setReadinessLoading(true);
    setReadinessError("");
    getOperatorReadiness()
      .then((result) => {
        setReadiness(result);
        setProvider(result.provider);
        setStorage(result.storage);
      })
      .catch((err: Error) => {
        setReadiness(null);
        setReadinessError(err.message);
      })
      .finally(() => setReadinessLoading(false));
  }, []);

  useEffect(() => {
    if (!shouldUseApi) return;
    setProviderLoading(true);
    getOperatorGenerationProvider()
      .then(setProvider)
      .catch(() => setProvider(null))
      .finally(() => setProviderLoading(false));
  }, []);

  useEffect(() => {
    if (!shouldUseApi) return;
    setStorageLoading(true);
    setStorageError("");
    getOperatorStorage()
      .then(setStorage)
      .catch((err: Error) => {
        setStorage(null);
        setStorageError(err.message);
      })
      .finally(() => setStorageLoading(false));
  }, []);

  useEffect(() => {
    if (!shouldUseApi) return;
    let mounted = true;
    setCostLoading(true);
    setCostError("");
    listOperatorGenerationCostsPage({
      provider: costFilters.provider || undefined,
      jobType: costFilters.jobType || undefined,
      status: costFilters.status || undefined,
      limit: 8,
      offset: 0,
    })
      .then((page) => {
        if (!mounted) return;
        setCostReport(page.data);
        setCostMeta(page.meta);
      })
      .catch((err: Error) => {
        if (!mounted) return;
        setCostReport(null);
        setCostMeta(null);
        setCostError(err.message);
      })
      .finally(() => {
        if (mounted) setCostLoading(false);
      });
    return () => {
      mounted = false;
    };
  }, [costFilters.provider, costFilters.jobType, costFilters.status]);

  return (
    <main className="operator-page">
      <PageHeader
        eyebrow="平台运营"
        title="平台市场管理"
        copy="平台模板和园所投稿内容的上架、下架和审核入口。"
        actions={<button className="button secondary" type="button" disabled={loading} onClick={refreshTemplates}>{loading ? "刷新中..." : "刷新模板"}</button>}
      />
      <Card>
        <div className="section-head">
          <div>
            <p className="eyebrow">试点就绪</p>
            <h2>部署前总检查</h2>
            <p>聚合数据库、真实生成、文件存储和预算上限，判断当前环境是否适合对外试点。</p>
          </div>
          <Badge tone={readiness?.ready ? "good" : readinessLoading ? "neutral" : "warn"}>
            {readinessLoading ? "检查中..." : readiness?.ready ? "已就绪" : "需处理"}
          </Badge>
        </div>
        {readinessError ? (
          <Notice title="试点就绪状态读取失败" copy={readinessError} tone="danger" />
        ) : readiness ? (
          <>
            <Notice
              title={readiness.ready ? "当前环境可进入试点" : "当前环境还不适合对外试点"}
              copy={readiness.ready ? "核心运行依赖已就绪，可以继续做真实 Seedream/DeepSeek 验收。" : "请先处理下方未通过检查项，再进入外部试点。"}
              tone={readiness.ready ? "good" : "warn"}
            />
            <div className="review-list">
              {readiness.checks.map((check) => (
                <div key={check.key}>
                  <span>{check.label}</span>
                  <strong>{check.ok ? "通过" : "需处理"}</strong>
                  <small>{check.message}</small>
                </div>
              ))}
            </div>
          </>
        ) : (
          <p className="task-summary">正在检查试点就绪状态。</p>
        )}
      </Card>
      <Card>
        <div className="section-head">
          <div>
            <p className="eyebrow">生成能力</p>
            <h2>当前生成 provider 状态</h2>
            <p>这块展示当前后端可用的生成模式，方便判断真实生成能力是否已经接入。</p>
          </div>
          <Badge tone={provider?.productionReady ? "good" : provider?.provider === "mock" ? "warn" : "info"}>
            {provider?.provider || (providerLoading ? "读取中..." : "未知")}
          </Badge>
        </div>
        {provider ? (
          <div className="review-list">
            <div><span>模式</span><strong>{provider.mode}</strong></div>
            <div><span>Schema</span><strong>{provider.schemaVersion}</strong></div>
            <div><span>文本任务</span><strong>{provider.supportsText.length ? provider.supportsText.join(" · ") : "暂无"}</strong></div>
            <div><span>图片任务</span><strong>{provider.supportsImage.length ? provider.supportsImage.join(" · ") : "暂无"}</strong></div>
            <div><span>需要密钥</span><strong>{provider.requiresApiKey ? "是" : "否"}</strong></div>
            <div><span>文本真实可用</span><strong>{provider.realTextReady ? "是" : "否"}</strong></div>
            <div><span>图片真实可用</span><strong>{provider.realImageReady ? "是" : "否"}</strong></div>
            <div><span>生产就绪</span><strong>{provider.productionReady ? "是" : "否"}</strong></div>
            <div><span>当前诊断</span><strong>{provider.diagnostic}</strong></div>
            <div><span>缺失配置</span><strong>{provider.missingConfiguration.length ? provider.missingConfiguration.join(" · ") : "无"}</strong></div>
            {provider.components.map((component) => (
              <div key={`${component.kind}-${component.provider}`}>
                <span>{componentKindLabel(component.kind)}组件</span>
                <strong>{component.provider} · {component.ready ? "已就绪" : `缺少 ${component.requiredConfiguration.join(" · ")}`}</strong>
                <small>{component.model} · {component.endpoint}</small>
              </div>
            ))}
          </div>
        ) : (
          <p className="task-summary">正在读取生成能力状态。</p>
        )}
        {provider && !provider.productionReady && (
          <Notice
            title="当前仍未生产就绪"
            copy={provider.diagnostic}
            tone={provider.provider === "mock" ? "warn" : "info"}
          />
        )}
      </Card>
      <Card>
        <div className="section-head">
          <div>
            <p className="eyebrow">文件存储</p>
            <h2>PDF 与插图存储边界</h2>
            <p>用于检查当前环境的导出文件目录、插图目录、大小上限和安全规则。</p>
          </div>
          <Badge tone={storage?.filenameValidation && storage?.sizeLimitEnabled ? "good" : "warn"}>
            {storageLoading ? "读取中..." : storage ? "已配置" : "未知"}
          </Badge>
        </div>
        {storageError ? (
          <Notice title="Storage 状态读取失败" copy={storageError} tone="danger" />
        ) : storage ? (
          <div className="review-list">
            <div><span>PDF 目录</span><strong>{storage.exportsDir}</strong></div>
            <div><span>插图目录</span><strong>{storage.generatedImagesDir}</strong></div>
            <div><span>存储后端</span><strong>{storageBackendLabel(storage.backend)}</strong></div>
            <div><span>下载策略</span><strong>{downloadStrategyLabel(storage.downloadStrategy)}</strong></div>
            <div><span>PDF 上限</span><strong>{formatBytes(storage.exportMaxBytes)}</strong></div>
            <div><span>插图上限</span><strong>{formatBytes(storage.generatedImageMaxBytes)}</strong></div>
            <div><span>文件名校验</span><strong>{storage.filenameValidation ? "已启用" : "未启用"}</strong></div>
            <div><span>大小限制</span><strong>{storage.sizeLimitEnabled ? "已启用" : "未启用"}</strong></div>
            <div><span>公共直链</span><strong>{storage.publicDirectAccess ? "允许" : "已关闭"}</strong></div>
          </div>
        ) : (
          <p className="task-summary">正在读取文件存储状态。</p>
        )}
        {storage && (!storage.filenameValidation || !storage.sizeLimitEnabled) && (
          <Notice
            title="Storage 安全边界不完整"
            copy="试点前建议启用文件名校验和大小上限，避免异常文件写入本地存储。"
            tone="warn"
          />
        )}
      </Card>
      <Card>
        <div className="section-head">
          <div>
            <p className="eyebrow">生成成本</p>
            <h2>AI 生成成本账本</h2>
            <p>按生成任务记录 provider、任务类型、估算 token/图片数量和成本，方便试点时控制预算。</p>
          </div>
          <Badge tone={costReport?.summary.totalJobs ? "info" : "neutral"}>
            {costLoading ? "读取中..." : `${costReport?.summary.totalJobs || 0} 个任务`}
          </Badge>
        </div>
        <div className="inline-actions">
          <label>
            Provider
            <select value={costFilters.provider} onChange={(event) => setCostFilters((current) => ({ ...current, provider: event.target.value }))}>
              <option value="">全部</option>
              <option value="deepseek">DeepSeek</option>
              <option value="seedream">Seedream</option>
              <option value="mock">Mock</option>
            </select>
          </label>
          <label>
            任务类型
            <select value={costFilters.jobType} onChange={(event) => setCostFilters((current) => ({ ...current, jobType: event.target.value }))}>
              <option value="">全部</option>
              <option value="storybook_plan">故事方案</option>
              <option value="storybook_roles">角色设定</option>
              <option value="storybook_pages">分页图文</option>
              <option value="customization_plan">定制方案</option>
              <option value="storybook_page_image">单页插图</option>
            </select>
          </label>
          <label>
            状态
            <select value={costFilters.status} onChange={(event) => setCostFilters((current) => ({ ...current, status: event.target.value }))}>
              <option value="">全部</option>
              <option value="succeeded">已成功</option>
              <option value="failed">已失败</option>
            </select>
          </label>
        </div>
        {costError ? (
          <Notice title="成本数据读取失败" copy={costError} tone="danger" />
        ) : costReport ? (
          <>
            {costReport.summary.budgetLimitMicros ? (
              <Notice
                title={
                  costReport.summary.budgetExceeded
                    ? "生成预算已达到上限"
                    : costReport.summary.budgetWarning
                      ? "生成预算接近上限"
                      : "生成预算运行中"
                }
                copy={
                  costReport.summary.budgetExceeded
                    ? "后端已暂停新建生成任务和失败任务重试；请提高预算上限或等待运营处理后再继续生成。"
                    : costReport.summary.budgetWarning
                      ? `当前已使用 ${formatBudgetPercent(costReport.summary.budgetUsedPercent)}，已达到 ${formatBudgetPercent(costReport.summary.budgetWarningPercent)} 预警线，请关注真实额度和后续生成计划。`
                      : `当前已使用 ${formatBudgetPercent(costReport.summary.budgetUsedPercent)}，达到上限后会暂停新建生成任务和失败任务重试。`
                }
                tone={costReport.summary.budgetExceeded ? "danger" : costReport.summary.budgetWarning ? "warn" : "info"}
              />
            ) : null}
            <section className="metric-grid">
              <div className="cost-metric">
                <span>估算总成本</span>
                <strong>{formatCostMicros(costReport.summary.totalCostMicros, costReport.summary.currency)}</strong>
                <p>包含当前筛选范围内的成功成本估算。</p>
              </div>
              <div className="cost-metric">
                <span>成功任务成本</span>
                <strong>{formatCostMicros(costReport.summary.succeededCostMicros, costReport.summary.currency)}</strong>
                <p>失败任务暂不计入实际成本。</p>
              </div>
              <div className="cost-metric">
                <span>文本估算 units</span>
                <strong>{costReport.summary.totalInputUnits + costReport.summary.totalOutputUnits}</strong>
                <p>输入和输出合计粗估。</p>
              </div>
              <div className="cost-metric">
                <span>图片数量</span>
                <strong>{costReport.summary.totalImages}</strong>
                <p>插图任务成功后计数。</p>
              </div>
              {costReport.summary.budgetLimitMicros ? (
                <div className={`cost-metric ${costReport.summary.budgetExceeded ? "cost-metric-danger" : ""}`}>
                  <span>预算使用率</span>
                  <strong>{formatBudgetPercent(costReport.summary.budgetUsedPercent)}</strong>
                  <p>预警线 {formatBudgetPercent(costReport.summary.budgetWarningPercent)}；上限 {formatCostMicros(costReport.summary.budgetLimitMicros, costReport.summary.currency)}</p>
                </div>
              ) : null}
            </section>
            {costReport.items.length ? (
              <div className="table-list">
                {costReport.items.map((item) => (
                  <div className="table-row" key={item.id}>
                    <div>
                      <strong>{generationJobTypeLabel(item.jobType)}</strong>
                      <span>{item.storybookTitle || item.workspaceName || "未关联绘本"}</span>
                    </div>
                    <Badge tone="info">{item.provider}</Badge>
                    <span>{item.imageCount ? `${item.imageCount} 张图` : `${item.estimatedInputUnits + item.estimatedOutputUnits} units`}</span>
                    <span>{formatCostMicros(item.estimatedCostMicros, item.currency)}</span>
                    <Badge tone={statusTone(item.status)}>{generationStatusLabel(item.status)}</Badge>
                  </div>
                ))}
              </div>
            ) : (
              <EmptyState title="暂无成本记录" copy={shouldUseApi ? "当前筛选条件下还没有生成成本日志。" : "连接后端后会显示真实生成成本。"} />
            )}
            {costMeta && costMeta.total > costReport.items.length && (
              <p className="task-summary">当前显示最近 {costReport.items.length} 条，共 {costMeta.total} 条；更多分页可在后续运营页展开。</p>
            )}
          </>
        ) : (
          <p className="task-summary">正在读取生成成本。</p>
        )}
      </Card>
      {notice && <Notice title="市场模板提示" copy={notice} tone="info" />}
      {initialLoading ? (
        <EmptyState title="正在加载市场模板" copy="正在读取已上架模板。" />
      ) : error && templates.length === 0 ? (
        <EmptyState title="市场模板加载失败" copy={error} />
      ) : (
        <>
          {error && <Notice title="市场模板更新失败" copy={error} tone="danger" />}
          <Card>
            <div className="section-head">
              <div>
                <p className="eyebrow">模板库</p>
                <h2>已显示 {templates.length}{shouldUseApi && templateMeta ? ` / 共 ${templateMeta.total}` : ""} 个模板</h2>
              </div>
              {shouldUseApi && templateMeta?.has_more ? (
                <button className="button secondary" type="button" disabled={loading} onClick={() => setTemplateOffset((value) => value + OPERATOR_PAGE_SIZE)}>
                  {loading ? "加载中..." : "继续加载模板"}
                </button>
              ) : (
                <Badge tone="info">{templates.length} 个</Badge>
              )}
            </div>
          </Card>
          <section className="storybook-grid">
            {templates.map((item) => (
              <Card key={item.id}>
                <Badge>{item.sourceLabel}</Badge>
                <h3>{item.title}</h3>
                <p>{item.summary}</p>
                <p className="task-summary">{item.supportsCustomization ? "可作为园所投稿模板或复制母本。" : "已上架模板，可直接复用。"}</p>
                <button
                  className="button secondary"
                  type="button"
                  onClick={() => openTemplateEditor(item)}
                >
                  编辑模板
                </button>
              </Card>
            ))}
          </section>
        </>
      )}
      {editingTemplate && (
        <Modal title="编辑市场模板" onClose={() => setEditingTemplate(null)}>
          <div className="form-grid">
            <label>模板标题<input value={templateForm.title} onChange={(event) => setTemplateForm((current) => ({ ...current, title: event.target.value }))} /></label>
            <label>年龄段<select value={templateForm.ageGroup} onChange={(event) => setTemplateForm((current) => ({ ...current, ageGroup: event.target.value }))}><option>3-4 岁</option><option>4-5 岁</option><option>5-6 岁</option></select></label>
            <label>使用场景<input value={templateForm.useScene} onChange={(event) => setTemplateForm((current) => ({ ...current, useScene: event.target.value }))} /></label>
            <label>标签<input value={templateForm.tags} onChange={(event) => setTemplateForm((current) => ({ ...current, tags: event.target.value }))} placeholder="分享、轮流、课堂共读" /></label>
            <label className="span-2">模板摘要<textarea rows={4} value={templateForm.summary} onChange={(event) => setTemplateForm((current) => ({ ...current, summary: event.target.value }))} /></label>
            <label className="check-row span-2"><input type="checkbox" checked={templateForm.supportsCustomization} onChange={(event) => setTemplateForm((current) => ({ ...current, supportsCustomization: event.target.checked }))} />支持继续派生定制绘本</label>
          </div>
          <div className="modal-actions">
            <button className="button secondary" type="button" onClick={() => setEditingTemplate(null)}>取消</button>
            <button className="button primary" type="button" disabled={savingTemplate} onClick={saveTemplateEdit}>{savingTemplate ? "保存中..." : "保存模板"}</button>
          </div>
        </Modal>
      )}
    </main>
  );
}

function componentKindLabel(kind: string) {
  return kind === "image" ? "图片" : kind === "text" ? "文本" : kind;
}

function formatBytes(value: number) {
  if (value === 0) return "不限";
  if (!Number.isFinite(value) || value < 0) return "未知";
  const mb = value / 1024 / 1024;
  if (mb >= 1) return `${Number.isInteger(mb) ? mb : mb.toFixed(1)} MB`;
  const kb = value / 1024;
  return `${Number.isInteger(kb) ? kb : kb.toFixed(1)} KB`;
}

function storageBackendLabel(value: string) {
  return value === "local" ? "本地文件系统" : value;
}

function downloadStrategyLabel(value: string) {
  return value === "authenticated_api" ? "权限 API 下载" : value;
}

export function OperatorSubmissionsPage() {
  const [reviewId, setReviewId] = useState<string | null>(null);
  const [notice, setNotice] = useState<{ title: string; copy: string; tone: "good" | "danger" } | null>(null);
  const [rows, setRows] = useState<MarketplaceSubmission[]>(shouldUseApi ? [] : submissions);
  const [offset, setOffset] = useState(0);
  const [statusFilter, setStatusFilter] = useState("");
  const [pageMeta, setPageMeta] = useState<PaginationMeta | null>(null);
  const [loading, setLoading] = useState(shouldUseApi);
  const [reviewing, setReviewing] = useState(false);
  const [rejecting, setRejecting] = useState(false);
  const [error, setError] = useState("");
  const [approvedTemplateId, setApprovedTemplateId] = useState<string | null>(null);
  const reviewItem = rows.find((item) => item.id === reviewId);
  const initialLoading = loading && (!shouldUseApi || rows.length === 0);

  useEffect(() => {
    setOffset(0);
  }, [statusFilter]);

  useEffect(() => {
    if (!shouldUseApi) return;
    let mounted = true;
    setLoading(true);
    if (offset === 0) {
      setRows([]);
      setPageMeta(null);
    }
    setError("");
    listOperatorSubmissionsPage({ status: statusFilter || undefined, limit: OPERATOR_PAGE_SIZE, offset })
      .then((page) => {
        if (!mounted) return;
        setRows((items) => (
          offset === 0
            ? page.data
            : [...items, ...page.data.filter((item) => !items.some((existing) => existing.id === item.id))]
        ));
        setPageMeta(page.meta);
        setError("");
      })
      .catch((err: Error) => {
        if (!mounted) return;
        if (offset === 0) {
          setRows([]);
          setPageMeta(null);
        }
        setError(err.message);
      })
      .finally(() => {
        if (mounted) setLoading(false);
      });
    return () => {
      mounted = false;
    };
  }, [offset, statusFilter]);

  async function approve() {
    if (!reviewItem) return;
    if (!shouldUseApi) {
      setRows((items) => items.map((item) => item.id === reviewItem.id ? { ...item, status: "listed" } : item));
      setReviewId(null);
      setNotice({ title: "审核动作已记录", copy: "投稿已通过审核，并进入市场上架流程。", tone: "good" });
      return;
    }
    setReviewing(true);
    setNotice(null);
    setApprovedTemplateId(null);
    try {
      const template = await approveOperatorSubmission(reviewItem.id);
      const updatedItem: MarketplaceSubmission = { ...reviewItem, status: "listed", updatedAt: "刚刚" };
      setRows((items) => (
        matchesOperatorSubmissionFilter(updatedItem, statusFilter)
          ? items.map((item) => item.id === reviewItem.id ? updatedItem : item)
          : items.filter((item) => item.id !== reviewItem.id)
      ));
      if (!matchesOperatorSubmissionFilter(updatedItem, statusFilter)) {
        setPageMeta((meta) => meta ? { ...meta, total: Math.max(0, meta.total - 1) } : meta);
      }
      setReviewId(null);
      setApprovedTemplateId(template.id);
      setNotice({ title: "已上架市场", copy: `《${template.title}》已成为市场模板，可被用户复制。`, tone: "good" });
    } catch (err) {
      setNotice({ title: "审核失败", copy: err instanceof Error ? err.message : "请稍后重试。", tone: "danger" });
    } finally {
      setReviewing(false);
    }
  }

  async function reject() {
    if (!reviewItem) return;
    if (!shouldUseApi) {
      setRows((items) => items.map((item) => item.id === reviewItem.id ? { ...item, status: "rejected", updatedAt: "刚刚" } : item));
      setReviewId(null);
      setNotice({ title: "已要求修改", copy: "投稿已标记为需要修改。", tone: "good" });
      return;
    }
    setRejecting(true);
    setNotice(null);
    try {
      const updated = await rejectOperatorSubmission(reviewItem.id);
      setRows((items) => (
        matchesOperatorSubmissionFilter(updated, statusFilter)
          ? items.map((item) => item.id === updated.id ? updated : item)
          : items.filter((item) => item.id !== updated.id)
      ));
      if (!matchesOperatorSubmissionFilter(updated, statusFilter)) {
        setPageMeta((meta) => meta ? { ...meta, total: Math.max(0, meta.total - 1) } : meta);
      }
      setReviewId(null);
      setNotice({ title: "已要求修改", copy: `《${updated.title}》已退回园所修改，暂不会进入市场。`, tone: "good" });
    } catch (err) {
      setNotice({ title: "退回失败", copy: err instanceof Error ? err.message : "请稍后重试。", tone: "danger" });
    } finally {
      setRejecting(false);
    }
  }

  return (
    <main className="operator-page">
      <PageHeader eyebrow="平台运营" title="平台投稿审核" copy="检查园所投稿是否适合公开进入绘本市场。" />
      {notice && (
        <Notice
          title={notice.title}
          copy={notice.copy}
          tone={notice.tone}
          action={approvedTemplateId ? <Link className="button secondary" to="/operator/marketplace">查看市场管理</Link> : undefined}
        />
      )}
      {initialLoading ? (
        <EmptyState title="正在加载投稿" copy="正在读取平台投稿审核队列。" />
      ) : error && rows.length === 0 ? (
        <EmptyState title="投稿加载失败" copy={error} />
      ) : (
        <Card>
          <div className="section-head">
            <div>
              <p className="eyebrow">审核队列</p>
              <h2>已显示 {rows.length}{shouldUseApi && pageMeta ? ` / 共 ${pageMeta.total}` : ""} 条投稿</h2>
            </div>
            <div className="inline-actions">
              <label>
                投稿状态
                <select value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}>
                  <option value="">全部</option>
                  <option value="submitted">待审核</option>
                  <option value="listed">已上架</option>
                  <option value="rejected">已退回</option>
                </select>
              </label>
              {shouldUseApi && pageMeta?.has_more ? (
                <button className="button secondary" type="button" disabled={loading} onClick={() => setOffset((value) => value + OPERATOR_PAGE_SIZE)}>
                  {loading ? "加载中..." : "继续加载审核队列"}
                </button>
              ) : (
                <Badge tone="info">{rows.length} 条</Badge>
              )}
            </div>
          </div>
          {error && <Notice title="审核队列更新失败" copy={error} tone="danger" />}
          <div className="table-list">
            {rows.map((item) => (
              <div className="table-row" key={item.id}>
                <div><strong>{item.title}</strong><span>{item.sourceStorybookTitle}</span></div>
                <span>{item.submittedBy}</span>
                <Badge tone={item.privacyConfirmed ? "good" : "danger"}>{item.privacyConfirmed ? "隐私已确认" : "隐私待确认"}</Badge>
                <Badge tone={statusTone(item.status)}>{submissionStatusLabel[item.status] || item.status}</Badge>
                <button className="button secondary" type="button" disabled={!item.privacyConfirmed || item.status === "listed" || item.status === "rejected"} onClick={() => setReviewId(item.id)}>
                  {item.status === "listed" ? "已上架" : item.status === "rejected" ? "已退回" : "审核"}
                </button>
              </div>
            ))}
          </div>
        </Card>
      )}
      <Card>
        <h2>状态说明</h2>
        <p className="task-summary">隐私待确认的投稿会先停在审核队列；通过后进入上架流程，成为市场模板后可被园所复制。</p>
      </Card>
      {reviewItem && (
        <Modal title={`审核《${reviewItem.title}》`} onClose={() => setReviewId(null)}>
          <p>检查内容是否适合公开进入绘本市场，并确认隐私风险已处理。</p>
          <div className="modal-actions">
            <button className="button secondary" type="button" disabled={rejecting || reviewing} onClick={reject}>{rejecting ? "退回中..." : "要求修改"}</button>
            <button className="button primary" type="button" disabled={reviewing || rejecting} onClick={approve}>{reviewing ? "上架中..." : "通过并上架市场"}</button>
          </div>
        </Modal>
      )}
    </main>
  );
}

function matchesOperatorSubmissionFilter(item: MarketplaceSubmission, statusFilter: string) {
  return !statusFilter || item.status === statusFilter;
}

function generationJobTypeLabel(jobType: string) {
  const labels: Record<string, string> = {
    storybook_plan: "故事方案",
    storybook_roles: "角色设定",
    storybook_pages: "分页图文",
    customization_plan: "定制方案",
    storybook_page_image: "单页插图",
  };
  return labels[jobType] || jobType;
}

function generationStatusLabel(status: string) {
  const labels: Record<string, string> = {
    succeeded: "已成功",
    failed: "已失败",
    queued: "排队中",
    running: "生成中",
  };
  return labels[status] || status;
}

function formatCostMicros(value: number, currency: string) {
  const amount = value / 1_000_000;
  const symbol = currency === "USD" ? "$" : `${currency} `;
  if (amount === 0) return `${symbol}0`;
  if (amount < 0.01) return `< ${symbol}0.01`;
  return `${symbol}${amount.toFixed(2)}`;
}

function formatBudgetPercent(value?: number) {
  if (typeof value !== "number" || !Number.isFinite(value)) return "0%";
  if (value >= 100) return `${Math.round(value)}%`;
  if (value > 0 && value < 1) return "< 1%";
  return `${Math.round(value)}%`;
}
