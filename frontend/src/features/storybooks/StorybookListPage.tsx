import { useEffect, useState } from "react";
import { Link, useOutletContext } from "react-router-dom";
import {
  listGenerationJobsPage,
  listStorybooksPage,
  shouldUseApi,
  type GenerationJob,
  type PaginationMeta,
} from "../../api/client";
import { Badge, Card, EmptyState, Notice, PageHeader, statusTone } from "../../components/ui";
import { storybooks } from "../../data/mock";
import type { Storybook, Workspace } from "../../types/domain";
import { generationJobStatusLabel, generationJobTypeLabel, storybookNextAction, storybookSourceLabel, storybookStatusLabel } from "../../utils/labels";

const PAGE_SIZE = 12;

export function StorybookListPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const [filter, setFilter] = useState<"all" | "plain" | "custom" | "exportable">("all");
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const [pageMeta, setPageMeta] = useState<PaginationMeta | null>(null);
  const [remoteBooks, setRemoteBooks] = useState<Storybook[]>([]);
  const [generationJobs, setGenerationJobs] = useState<GenerationJob[]>([]);
  const [loading, setLoading] = useState(shouldUseApi);
  const [error, setError] = useState("");
  const books = shouldUseApi ? remoteBooks : storybooks.filter((item) => item.workspaceId === workspace.id);
  const initialLoading = loading && (!shouldUseApi || remoteBooks.length === 0);
  const filteredBooks = shouldUseApi ? books : books.filter((book) => {
    const matchesFilter =
      filter === "all" ||
      (filter === "plain" && book.type === "plain") ||
      (filter === "custom" && book.type === "custom") ||
      (filter === "exportable" && book.status === "exportable");
    const text = `${book.title} ${book.teachingGoal} ${book.useScene} ${book.ageGroup}`.toLowerCase();
    return matchesFilter && text.includes(query.trim().toLowerCase());
  });
  const filterItems = [
    ["all", "全部"],
    ["plain", "普通绘本"],
    ["custom", "定制绘本"],
    ["exportable", "可导出"],
  ] as const;
  const pendingGenerationCount = generationJobs.filter((job) => job.status === "queued" || job.status === "running" || job.status === "failed").length;
  const summaryItems = [
    { label: "普通绘本", value: books.filter((book) => book.type === "plain").length, copy: "可作为定制绘本母本", tone: "info" as const },
    { label: "定制绘本", value: books.filter((book) => book.type === "custom").length, copy: "服务单个儿童", tone: "good" as const },
    { label: "可导出", value: books.filter((book) => book.status === "exportable").length, copy: "可分享或继续定制", tone: "good" as const },
    { label: "待处理任务", value: pendingGenerationCount, copy: "排队、运行或失败任务", tone: pendingGenerationCount ? "warn" as const : "neutral" as const },
  ];

  function recentTaskCopy(book: Storybook) {
    const recent = generationJobs
      .filter((job) => job.storybookId === book.id)
      .sort((left, right) => right.createdAt.localeCompare(left.createdAt))[0];
    if (!recent) return null;
    return `最近任务：${generationJobTypeLabel[recent.jobType] || recent.jobType} · ${generationJobStatusLabel[recent.status] || recent.status} · ${recent.finishedAt || recent.createdAt}`;
  }

  useEffect(() => {
    if (!shouldUseApi) return;
    let mounted = true;
    setLoading(true);
    if (offset === 0) {
      setRemoteBooks([]);
      setGenerationJobs([]);
      setPageMeta(null);
    }
    setError("");
    const type = filter === "plain" || filter === "custom" ? filter : undefined;
    const status = filter === "exportable" ? "exportable" : undefined;
    Promise.all([
      listStorybooksPage(workspace.id, { type, status, q: query.trim(), limit: PAGE_SIZE, offset }),
      listGenerationJobsPage(workspace.id, { limit: 50, offset: 0 }),
    ])
      .then(([page, jobsPage]) => {
        if (!mounted) return;
        setRemoteBooks((current) => (
          offset === 0
            ? page.data
            : [...current, ...page.data.filter((book) => !current.some((item) => item.id === book.id))]
        ));
        setPageMeta(page.meta);
        setGenerationJobs(jobsPage.data);
        setError("");
      })
      .catch((err) => {
        if (!mounted) return;
        if (offset === 0) {
          setRemoteBooks([]);
          setGenerationJobs([]);
          setPageMeta(null);
        }
        setError(err instanceof Error ? err.message : "无法读取绘本列表");
      })
      .finally(() => {
        if (mounted) setLoading(false);
      });
    return () => {
      mounted = false;
    };
  }, [filter, offset, query, workspace.id]);

  return (
    <div className="page-stack">
      <PageHeader
        eyebrow={workspace.type === "personal" ? "我的绘本" : "园所绘本"}
        title={workspace.type === "personal" ? "我的绘本" : "园所绘本"}
        copy="查看普通绘本、定制绘本、生成状态、导出和市场投稿状态。"
        actions={<Link className="button primary" to="new">新建普通绘本</Link>}
      />
      {error && filteredBooks.length > 0 && <Notice title="列表更新失败" copy={error} tone="danger" />}
      <section className="list-hero">
        <div>
          <Badge tone="info">创作入口</Badge>
          <h2>先创建普通绘本，再派生定制绘本</h2>
          <p>普通绘本适合班级共读和主题活动；完成后可选择孩子生成独立定制副本。</p>
        </div>
        <div className="inline-actions">
          <Link className="button primary" to="new">创建普通绘本</Link>
          <Link className="button secondary" to="../marketplace">从市场复制</Link>
        </div>
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
            placeholder="搜索标题、主题或教学目标"
          />
        </div>
      </Card>
      <section className="metric-grid">
        {summaryItems.map((item) => (
          <Card key={item.label}>
            <Badge tone={item.tone}>{item.label}</Badge>
            <strong>{item.value}</strong>
            <p>{item.copy}</p>
          </Card>
        ))}
      </section>
      {initialLoading ? (
        <EmptyState title="正在读取绘本" copy="正在从后端加载当前空间的绘本列表。" />
      ) : error && filteredBooks.length === 0 ? (
        <EmptyState title="绘本列表加载失败" copy={error} />
      ) : books.length === 0 ? (
        <EmptyState title="当前空间还没有绘本" copy="先创建一本普通绘本，或从市场复制一个模板。" />
      ) : filteredBooks.length === 0 ? (
        <EmptyState title="没有匹配的绘本" copy="换一个筛选条件，或清空搜索关键词后再试。" action={<button className="button secondary" type="button" onClick={() => { setFilter("all"); setQuery(""); }}>清空筛选</button>} />
      ) : (
        <>
          {shouldUseApi && pageMeta && (
            <Card>
              <div className="section-head">
                <div>
                  <p className="eyebrow">列表结果</p>
                  <h2>已显示 {filteredBooks.length} / 共 {pageMeta.total} 本</h2>
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
            {filteredBooks.map((book) => (
              <Link className="storybook-card" to={book.id} key={book.id}>
                <div className="cover-art"><span>{book.coverTone}</span><strong>{book.title.slice(0, 2)}</strong></div>
                <div className="storybook-card-body">
                  <div className="card-line"><Badge tone={book.type === "plain" ? "info" : "good"}>{book.type === "plain" ? "普通绘本" : "定制绘本"}</Badge><Badge tone={statusTone(book.status)}>{storybookStatusLabel[book.status]}</Badge></div>
                  <h3>{book.title}</h3>
                  <p>{book.teachingGoal}</p>
                  <p className="next-action">{storybookSourceLabel(book)}</p>
                  <p className="next-action">{storybookNextAction(book)}</p>
                  {shouldUseApi && recentTaskCopy(book) && <p className="task-summary">{recentTaskCopy(book)}</p>}
                  <div className="meta-line"><span>{book.ageGroup}</span><span>{book.useScene}</span><span>{book.updatedAt}</span></div>
                </div>
              </Link>
            ))}
          </section>
        </>
      )}
    </div>
  );
}
