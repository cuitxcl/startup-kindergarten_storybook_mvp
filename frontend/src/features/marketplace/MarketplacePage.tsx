import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  listMarketplaceTemplatesPage,
  shouldUseApi,
  type PaginationMeta,
} from "../../api/client";
import { Badge, Card, EmptyState, Notice, PageHeader } from "../../components/ui";
import { marketplaceTemplates } from "../../data/mock";
import type { MarketplaceTemplate } from "../../types/domain";

const PAGE_SIZE = 12;

export function MarketplacePage() {
  const [filter, setFilter] = useState<"all" | "platform" | "school_submission" | "customizable">("all");
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const [pageMeta, setPageMeta] = useState<PaginationMeta | null>(null);
  const [remoteTemplates, setRemoteTemplates] = useState<MarketplaceTemplate[]>([]);
  const [loading, setLoading] = useState(shouldUseApi);
  const [error, setError] = useState("");
  const templates = shouldUseApi ? remoteTemplates : marketplaceTemplates;
  const initialLoading = loading && (!shouldUseApi || remoteTemplates.length === 0);
  const filteredTemplates = shouldUseApi ? templates : templates.filter((template) => {
    const matchesFilter =
      filter === "all" ||
      template.sourceType === filter ||
      (filter === "customizable" && template.supportsCustomization);
    const text = `${template.title} ${template.summary} ${template.ageGroup} ${template.useScene} ${template.tags.join(" ")}`.toLowerCase();
    return matchesFilter && text.includes(query.trim().toLowerCase());
  });
  const recommendedId = filteredTemplates[0]?.id || templates[0]?.id || "template-1";
  const summaryItems = [
    { label: "模板总数", value: templates.length, copy: "当前空间可见的绘本模板", tone: "info" as const },
    { label: "平台精选", value: templates.filter((template) => template.sourceType === "platform").length, copy: "适合直接复制使用", tone: "good" as const },
    { label: "园所投稿", value: templates.filter((template) => template.sourceType === "school_submission").length, copy: "来自园所的优秀作品", tone: "neutral" as const },
    { label: "支持定制", value: templates.filter((template) => template.supportsCustomization).length, copy: "可继续派生定制绘本", tone: "warn" as const },
  ];
  const filterItems = [
    ["all", "全部"],
    ["platform", "平台精选"],
    ["school_submission", "园所投稿"],
    ["customizable", "支持定制"],
  ] as const;

  const loadTemplates = () => {
    if (!shouldUseApi) return;
    setLoading(true);
    if (offset === 0) {
      setRemoteTemplates([]);
      setPageMeta(null);
    }
    setError("");
    const source = filter === "platform" || filter === "school_submission" ? filter : undefined;
    const supportsCustomization = filter === "customizable" ? true : undefined;
    listMarketplaceTemplatesPage({
      source,
      supportsCustomization,
      q: query.trim(),
      limit: PAGE_SIZE,
      offset,
    })
      .then((page) => {
        setRemoteTemplates((current) => (
          offset === 0
            ? page.data
            : [...current, ...page.data.filter((template) => !current.some((item) => item.id === template.id))]
        ));
        setPageMeta(page.meta);
      })
      .catch((err: Error) => {
        if (offset === 0) {
          setRemoteTemplates([]);
          setPageMeta(null);
        }
        setError(err.message);
      })
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    loadTemplates();
  }, [filter, offset, query]);

  return (
    <div className="page-stack">
      <PageHeader eyebrow="绘本市场" title="绘本市场" copy="平台精选和园所投稿的可复用绘本模板库。" />
      {error && filteredTemplates.length > 0 && <Notice title="模板更新失败" copy={error} tone="danger" />}
      <section className="list-hero">
        <div>
          <Badge tone="info">模板复用</Badge>
          <h2>从成熟模板开始，复制后再编辑</h2>
          <p>适合快速创建普通绘本，后续可继续派生定制绘本。</p>
        </div>
        <div className="inline-actions">
          <Link className="button primary" to={recommendedId}>查看推荐模板</Link>
          <Link className="button secondary" to="../storybooks">回到绘本列表</Link>
        </div>
      </section>
      <section className="metric-grid">
        {summaryItems.map((item) => (
          <Card key={item.label}>
            <Badge tone={item.tone}>{item.label}</Badge>
            <strong>{item.value}</strong>
            <p>{item.copy}</p>
          </Card>
        ))}
      </section>
      <Card>
        <div className="filter-row">
          {filterItems.map(([value, label]) => (
            <button
              key={value}
              type="button"
              className={`filter ${filter === value ? "active" : ""}`}
              onClick={() => {
                setFilter(value);
                setOffset(0);
              }}
            >
              {label}
            </button>
          ))}
          <input
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setOffset(0);
            }}
            placeholder="搜索主题、年龄段或使用场景"
          />
        </div>
      </Card>
      {initialLoading ? (
        <EmptyState title="正在加载模板" copy="正在从后端读取绘本市场模板。" />
      ) : error && filteredTemplates.length === 0 ? (
        <EmptyState title="模板加载失败" copy={error} action={<button className="button secondary" type="button" onClick={loadTemplates}>重新加载</button>} />
      ) : filteredTemplates.length === 0 ? (
        <EmptyState title="没有匹配的模板" copy="换一个筛选条件，或清空搜索关键词后再试。" action={<button className="button secondary" type="button" onClick={() => { setFilter("all"); setQuery(""); }}>清空筛选</button>} />
      ) : (
        <>
          {shouldUseApi && pageMeta && (
            <Card>
              <div className="section-head">
                <div>
                  <p className="eyebrow">市场结果</p>
                  <h2>已显示 {filteredTemplates.length} / 共 {pageMeta.total} 个模板</h2>
                </div>
                {pageMeta.has_more && (
                  <button className="button secondary" type="button" disabled={loading} onClick={() => setOffset((value) => value + PAGE_SIZE)}>
                    {loading ? "加载中..." : "继续加载"}
                  </button>
                )}
              </div>
            </Card>
          )}
          <section className="storybook-grid">
            {filteredTemplates.map((template) => (
              <Link className="storybook-card" to={template.id} key={template.id}>
                <div className="cover-art market"><span>{template.sourceLabel}</span><strong>{template.title.slice(0, 2)}</strong></div>
                <div className="storybook-card-body">
                  <div className="card-line"><Badge tone={template.sourceType === "platform" ? "info" : "good"}>{template.sourceLabel}</Badge><Badge>{template.pageCount} 页</Badge></div>
                  <h3>{template.title}</h3>
                  <p>{template.summary}</p>
                  <p className="task-summary">{template.supportsCustomization ? "复制后可继续作为定制绘本母本。" : "复制后可直接作为普通绘本使用。"}</p>
                  <div className="meta-line"><span>{template.ageGroup}</span><span>{template.useScene}</span><span>{template.supportsCustomization ? "可定制" : "不可定制"}</span></div>
                </div>
              </Link>
            ))}
          </section>
        </>
      )}
    </div>
  );
}
