import { useEffect, useState } from "react";
import { Link, useOutletContext } from "react-router-dom";
import {
  listGenerationJobsPage,
  listStorybooksPage,
  type GenerationJob,
  type PaginationMeta,
} from "../../api/client";
import { Badge, Card, EmptyState, Notice, PageHeader, statusTone } from "../../components/ui";
import type { Storybook, StorybookRole, Workspace } from "../../types/domain";
import { generationJobStatusLabel, generationJobTypeLabel, storybookNextAction, storybookSourceLabel, storybookStatusLabel } from "../../utils/labels";
import { useDebouncedValue } from "../../utils/useDebouncedValue";

const PAGE_SIZE = 12;

export function StorybookListPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const [filter, setFilter] = useState<"all" | "plain" | "custom" | "exportable">("all");
  const [query, setQuery] = useState("");
  const debouncedQuery = useDebouncedValue(query.trim(), 300);
  const [offset, setOffset] = useState(0);
  const [pageMeta, setPageMeta] = useState<PaginationMeta | null>(null);
  const [remoteBooks, setRemoteBooks] = useState<Storybook[]>([]);
  const [generationJobs, setGenerationJobs] = useState<GenerationJob[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const books = remoteBooks;
  const initialLoading = loading && remoteBooks.length === 0;
  const filteredBooks = books;
  const filterItems = [
    ["all", "全部"],
    ["plain", "普通绘本"],
    ["custom", "定制绘本"],
    ["exportable", "可导出"],
  ] as const;
  const pendingGenerationCount = generationJobs.filter((job) => job.status === "queued" || job.status === "running" || job.status === "failed").length;
  function recentTaskCopy(book: Storybook) {
    const recent = generationJobs
      .filter((job) => job.storybookId === book.id)
      .sort((left, right) => right.createdAt.localeCompare(left.createdAt))[0];
    if (!recent) return null;
    return `最近任务：${generationJobTypeLabel[recent.jobType] || recent.jobType} · ${generationJobStatusLabel[recent.status] || recent.status} · ${recent.finishedAt || recent.createdAt}`;
  }

  useEffect(() => {
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
      listStorybooksPage(workspace.id, { type, status, q: debouncedQuery, limit: PAGE_SIZE, offset }),
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
  }, [filter, offset, debouncedQuery, workspace.id]);

  return (
    <div className="page-stack">
      <PageHeader
        eyebrow={workspace.type === "personal" ? "我的绘本" : "园所绘本"}
        title={workspace.type === "personal" ? "我的绘本" : "园所绘本"}
        copy="普通绘本用于班级共读，也可以继续派生儿童定制绘本。"
        actions={
          <>
            <Link className="button secondary" to="../marketplace">从市场复制</Link>
            <Link className="button primary" to="new">新建普通绘本</Link>
          </>
        }
      />
      {error && filteredBooks.length > 0 && <Notice title="列表更新失败" copy={error} tone="danger" />}
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
      {pendingGenerationCount > 0 && (
        <Notice
          title={`有 ${pendingGenerationCount} 条生成任务需要关注`}
          copy="包含排队、运行中或失败的任务。打开对应绘本详情可以继续处理。"
          tone="warn"
        />
      )}
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
          {pageMeta && (
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
            {filteredBooks.map((book) => {
              const customizationBlocker = book.type === "plain" ? customizationBlockerForList(book, generationJobs) : "";
              const exportBlocker = exportBlockerForList(book, generationJobs);

              return (
                <article className="storybook-card" key={book.id}>
                  <div className="cover-art"><span>{book.coverTone}</span><strong>{book.title.slice(0, 2)}</strong></div>
                  <div className="storybook-card-body">
                    <div className="card-line"><Badge tone={book.type === "plain" ? "info" : "good"}>{book.type === "plain" ? "普通绘本" : "定制绘本"}</Badge><Badge tone={statusTone(book.status)}>{storybookStatusLabel[book.status]}</Badge></div>
                    <h3>{book.title}</h3>
                    <p>{book.teachingGoal}</p>
                    <p className="next-action">{storybookSourceLabel(book)}</p>
                    <p className="next-action">{storybookNextAction(book)}</p>
                    {recentTaskCopy(book) && <p className="task-summary">{recentTaskCopy(book)}</p>}
                    {customizationBlocker && <p className="task-summary">定制条件：{customizationBlocker}</p>}
                    {book.type === "custom" && exportBlocker && <p className="task-summary">导出条件：{exportBlocker}</p>}
                    <div className="meta-line"><span>{book.ageGroup}</span><span>{book.useScene}</span><span>{book.updatedAt}</span></div>
                    <div className="storybook-card-actions">
                      {book.type === "plain" ? (
                        <>
                          {customizationBlocker ? (
                            <Link className="button primary" to={book.id}>继续完成</Link>
                          ) : (
                            <Link className="button primary" to={`${book.id}/customize`}>生成定制版</Link>
                          )}
                          <Link className="button secondary" to={book.id}>查看详情</Link>
                        </>
                      ) : (
                        <>
                          <Link className="button primary" to={book.id}>继续编辑</Link>
                          <Link className="button secondary" to={book.id}>{exportBlocker ? "查看详情" : "导出或分享"}</Link>
                        </>
                      )}
                    </div>
                  </div>
                </article>
              );
            })}
          </section>
        </>
      )}
    </div>
  );
}

function customizationBlockerForList(book: Storybook, jobs: GenerationJob[]) {
  if (book.type !== "plain") return "";
  return exportBlockerForList(book, jobs);
}

function exportBlockerForList(book: Storybook, jobs: GenerationJob[]) {
  if (!book.pages.length) return "请先生成绘本分页";
  if (!book.roles.length) return "请先确认角色与道具";

  const activeJob = jobs.find((job) => (
    job.storybookId === book.id && (job.status === "queued" || job.status === "running")
  ));
  if (activeJob) return `${generationJobTypeLabel[activeJob.jobType] || activeJob.jobType}仍在生成，请完成后再定制`;

  const failedPages = book.pages.filter((page) => page.status === "failed");
  if (failedPages.length) return `仍有 ${failedPages.length} 页插图生成失败，请先处理`;

  const generatingPages = book.pages.filter((page) => page.status === "generating");
  if (generatingPages.length) return "仍有分页插图正在生成，请完成后再生成定制版";

  const redrawPages = book.pages.filter((page) => page.status === "needs_regeneration");
  if (redrawPages.length) return `仍有 ${redrawPages.length} 页需要重绘，请先完成普通绘本`;

  const missingReferences = book.roles.filter((role) => roleNeedsReferenceForList(book, role) && (role.referenceStatus !== "ready" || !role.referenceImageUrl));
  if (missingReferences.length) return `跨页角色参考图未完成：${missingReferences.map((role) => role.name).join("、")}`;

  if (book.quality?.status === "blocked") return "质量检查存在阻断项，请先修正";
  if (book.status !== "exportable" && book.status !== "listed") return `当前状态为「${storybookStatusLabel[book.status] || "未完成"}」，完成后才能继续`;
  return "";
}

function roleNeedsReferenceForList(book: Storybook, role: StorybookRole) {
  return role.needsConsistency && book.pages.filter((page) => {
    const text = `${page.title} ${page.body} ${page.illustrationPrompt}`;
    return text.includes(role.name);
  }).length >= 2;
}
