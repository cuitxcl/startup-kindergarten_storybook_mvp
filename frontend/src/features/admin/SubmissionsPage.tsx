import { useEffect, useMemo, useState } from "react";
import { useOutletContext } from "react-router-dom";
import {
  confirmSubmissionPrivacy,
  createSubmission,
  isApiClientError,
  listStorybooksPage,
  listSubmissionsPage,
  shouldUseApi,
  type PaginationMeta,
} from "../../api/client";
import { Badge, Card, EmptyState, Modal, Notice, PageHeader, statusTone } from "../../components/ui";
import { submissions } from "../../data/mock";
import type { MarketplaceSubmission, Storybook, Workspace } from "../../types/domain";
import { submissionStatusLabel } from "../../utils/labels";

const PAGE_SIZE = 12;
const PLAIN_BOOK_PAGE_SIZE = 20;

export function SubmissionsPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const [open, setOpen] = useState<"new" | "privacy" | null>(null);
  const [selectedSubmissionId, setSelectedSubmissionId] = useState<string | null>(null);
  const [selectedStorybookId, setSelectedStorybookId] = useState("");
  const [notice, setNotice] = useState<{ title: string; copy: string } | null>(null);
  const [remoteSubmissions, setRemoteSubmissions] = useState<MarketplaceSubmission[]>([]);
  const [offset, setOffset] = useState(0);
  const [statusFilter, setStatusFilter] = useState("");
  const [pageMeta, setPageMeta] = useState<PaginationMeta | null>(null);
  const [plainBooks, setPlainBooks] = useState<Storybook[]>([]);
  const [plainBookMeta, setPlainBookMeta] = useState<PaginationMeta | null>(null);
  const [plainBookLoading, setPlainBookLoading] = useState(false);
  const [loading, setLoading] = useState(shouldUseApi);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState("");
  const rows = shouldUseApi ? remoteSubmissions : submissions.filter((item) => item.workspaceId === workspace.id);
  const initialLoading = loading && (!shouldUseApi || remoteSubmissions.length === 0);
  const selectableBooks = useMemo(() => {
    const submittedTitles = new Set(rows.map((item) => item.sourceStorybookTitle));
    return (shouldUseApi
      ? plainBooks
      : [
          { id: "storybook-3", title: "午睡小小约定" },
          { id: "storybook-1", title: "排队像小火车" },
        ]).filter((book) => !submittedTitles.has(book.title));
  }, [plainBooks, rows]);
  const selectedSubmission = rows.find((item) => item.id === selectedSubmissionId);
  const nextPrivacySubmission = rows.find((item) => !item.privacyConfirmed);

  useEffect(() => {
    setOffset(0);
  }, [workspace.id]);

  useEffect(() => {
    setOffset(0);
  }, [statusFilter]);

  useEffect(() => {
    if (!shouldUseApi) return;
    let mounted = true;
    setLoading(true);
    setError("");
    if (offset === 0) {
      setRemoteSubmissions([]);
      setPlainBooks([]);
      setPageMeta(null);
      setPlainBookMeta(null);
    }
    Promise.all([
      listSubmissionsPage(workspace.id, { status: statusFilter || undefined, limit: PAGE_SIZE, offset }),
      offset === 0 ? listStorybooksPage(workspace.id, { type: "plain", limit: PLAIN_BOOK_PAGE_SIZE, offset: 0 }) : Promise.resolve(null),
    ])
      .then(([page, booksPage]) => {
        if (!mounted) return;
        setRemoteSubmissions((current) => (
          offset === 0
            ? page.data
            : [...current, ...page.data.filter((item) => !current.some((currentItem) => currentItem.id === item.id))]
        ));
        setPageMeta(page.meta);
        if (booksPage) {
          setPlainBooks(booksPage.data);
          setPlainBookMeta(booksPage.meta);
          setSelectedStorybookId((value) => value || booksPage.data[0]?.id || "");
        }
      })
      .catch((err: Error) => {
        if (!mounted) return;
        if (offset === 0) {
          setRemoteSubmissions([]);
          setPlainBooks([]);
          setPageMeta(null);
          setPlainBookMeta(null);
        }
        setError(err.message);
      })
      .finally(() => {
        if (mounted) setLoading(false);
      });
    return () => {
      mounted = false;
    };
  }, [offset, statusFilter, workspace.id]);

  useEffect(() => {
    if (selectableBooks.length === 0) {
      setSelectedStorybookId("");
      return;
    }
    if (!selectableBooks.some((book) => book.id === selectedStorybookId)) {
      setSelectedStorybookId(selectableBooks[0].id);
    }
  }, [selectableBooks, selectedStorybookId]);

  const submitNew = async () => {
    if (!selectedStorybookId) {
      setNotice({ title: "无法创建投稿", copy: "请先选择一本普通绘本。" });
      return;
    }
    if (!shouldUseApi) {
      setOpen(null);
      setNotice({ title: "投稿草稿已创建", copy: "这是 mock 反馈：真实接入后会进入投稿预览和隐私确认流程。" });
      return;
    }
    setSubmitting(true);
    setNotice(null);
    try {
      const item = await createSubmission(workspace.id, selectedStorybookId);
      if (matchesSubmissionFilter(item, statusFilter)) {
        setRemoteSubmissions((current) => [item, ...current]);
        setPageMeta((meta) => meta ? { ...meta, total: meta.total + 1 } : meta);
      }
      setOpen(null);
      setNotice({ title: "投稿草稿已创建", copy: `《${item.title}》已进入投稿隐私确认流程。` });
    } catch (err) {
      setNotice({ title: "创建失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setSubmitting(false);
    }
  };

  const confirmPrivacy = async () => {
    if (!selectedSubmission) return;
    if (!shouldUseApi) {
      setOpen(null);
      setNotice({ title: "隐私确认已保存", copy: "这是 mock 反馈：真实接入后投稿会继续进入平台审核。" });
      return;
    }
    setSubmitting(true);
    setNotice(null);
    try {
      const updated = await confirmSubmissionPrivacy(workspace.id, selectedSubmission.id);
      setRemoteSubmissions((items) => (
        matchesSubmissionFilter(updated, statusFilter)
          ? items.map((item) => item.id === updated.id ? updated : item)
          : items.filter((item) => item.id !== updated.id)
      ));
      if (!matchesSubmissionFilter(updated, statusFilter)) {
        setPageMeta((meta) => meta ? { ...meta, total: Math.max(0, meta.total - 1) } : meta);
      }
      setPlainBooks((items) => items.map((book) => (
        book.title === updated.sourceStorybookTitle
          ? { ...book, status: "submitted", visibility: "market_submission" }
          : book
      )));
      setOpen(null);
      setSelectedSubmissionId(null);
      setNotice({ title: "隐私确认已保存", copy: `《${updated.title}》已进入平台审核队列。` });
    } catch (err) {
      if (isApiClientError(err) && err.code === "state_conflict" && err.message.includes("投稿内容可能包含")) {
        setNotice({
          title: "确认失败：发现隐私风险",
          copy: `${err.message}。请回到绘本详情修改对应正文、插图描述或角色设定，再重新确认投稿隐私。`,
        });
      } else {
        setNotice({ title: "确认失败", copy: err instanceof Error ? err.message : "请稍后重试" });
      }
    } finally {
      setSubmitting(false);
    }
  };

  const loadMorePlainBooks = async () => {
    if (!shouldUseApi || !plainBookMeta?.has_more) return;
    setPlainBookLoading(true);
    setNotice(null);
    try {
      const nextOffset = plainBookMeta.offset + plainBookMeta.limit;
      const page = await listStorybooksPage(workspace.id, { type: "plain", limit: PLAIN_BOOK_PAGE_SIZE, offset: nextOffset });
      setPlainBooks((items) => [
        ...items,
        ...page.data.filter((book) => !items.some((item) => item.id === book.id)),
      ]);
      setPlainBookMeta(page.meta);
    } catch (err) {
      setNotice({ title: "普通绘本加载失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setPlainBookLoading(false);
    }
  };

  return (
    <div className="page-stack">
      <PageHeader eyebrow="市场投稿" title="市场投稿" copy="园所普通绘本投稿前必须经过预览和隐私确认。" actions={<button className="button primary" type="button" onClick={() => setOpen("new")}>新建投稿</button>} />
      {notice && <Notice title={notice.title} copy={notice.copy} tone={notice.title.includes("失败") || notice.title.includes("无法") ? "danger" : "good"} />}
      {initialLoading && <EmptyState title="正在加载投稿" copy="正在读取园所投稿队列和可投稿绘本。" />}
      {error && rows.length > 0 && <Notice title="投稿列表更新失败" copy={error} tone="danger" />}
      {error && rows.length === 0 && <EmptyState title="投稿加载失败" copy={error} />}
      <Card>
        <div className="section-head">
          <div><p className="eyebrow">审核队列</p><h2>投稿与隐私状态</h2></div>
          <div className="inline-actions">
            <label>
              投稿状态
              <select value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}>
                <option value="">全部</option>
                <option value="draft">隐私待确认</option>
                <option value="submitted">审核中</option>
                <option value="listed">已上架</option>
                <option value="rejected">已退回</option>
              </select>
            </label>
            <Badge tone="warn">{shouldUseApi && pageMeta ? `已显示 ${rows.length} / 共 ${pageMeta.total} 条` : "优先处理隐私风险"}</Badge>
          </div>
        </div>
        {shouldUseApi && pageMeta?.has_more && (
          <div className="inline-actions">
            <button className="button secondary" type="button" disabled={loading} onClick={() => setOffset((value) => value + PAGE_SIZE)}>
              {loading ? "加载中..." : "继续加载投稿"}
            </button>
          </div>
        )}
        {!initialLoading && rows.length === 0 ? (
          <EmptyState title="还没有市场投稿" copy="选择一本普通绘本创建投稿草稿，再完成隐私确认。" />
        ) : rows.length > 0 ? (
          <div className="table-list">
          {rows.map((item) => (
            <div className="table-row" key={item.id}>
              <div><strong>{item.title}</strong><span>来源：{item.sourceStorybookTitle}</span></div>
              <span>{item.submittedBy}</span>
              <Badge tone={item.privacyConfirmed ? "good" : "danger"}>{item.privacyConfirmed ? "隐私已确认" : "隐私待确认"}</Badge>
              <Badge tone={statusTone(item.status)}>{submissionStatusLabel[item.status] || item.status}</Badge>
              <button className="button secondary" type="button" onClick={() => { setSelectedSubmissionId(item.id); setOpen("privacy"); }}>{item.privacyConfirmed ? "查看确认" : "确认隐私"}</button>
            </div>
          ))}
          </div>
        ) : null}
      </Card>
      <Card>
        <h2>投稿隐私检查</h2>
        <div className="check-list">
          <label><input type="checkbox" defaultChecked />不是定制绘本</label>
          <label><input type="checkbox" defaultChecked />不含儿童照片</label>
          <label><input type="checkbox" defaultChecked />不含具体儿童姓名、家庭信息或个别行为描述</label>
        </div>
        <button
          className="button primary"
          type="button"
          onClick={() => {
            if (!shouldUseApi) {
              setNotice({ title: "隐私检查已确认", copy: "这是 mock 反馈：真实接入后会记录确认人、时间和检查项。" });
              return;
            }
            if (!nextPrivacySubmission) {
              setNotice({ title: "暂无待确认投稿", copy: "当前投稿都已完成隐私确认；新建投稿后再进行检查。" });
              return;
            }
            setSelectedSubmissionId(nextPrivacySubmission.id);
            setOpen("privacy");
          }}
        >
          {nextPrivacySubmission ? "处理下一条隐私确认" : "暂无待确认投稿"}
        </button>
      </Card>
      {open === "new" && (
        <Modal title="新建市场投稿" onClose={() => setOpen(null)}>
          {selectableBooks.length === 0 ? (
            <EmptyState title="没有可投稿的普通绘本" copy="普通绘本已经投稿过，或当前空间还没有可投稿的普通绘本。" />
          ) : (
            <label>选择普通绘本<select value={selectedStorybookId} onChange={(event) => setSelectedStorybookId(event.target.value)}>{selectableBooks.map((book) => <option key={book.id} value={book.id}>{book.title}</option>)}</select></label>
          )}
          {shouldUseApi && plainBookMeta?.has_more && (
            <button className="button secondary" type="button" disabled={plainBookLoading} onClick={loadMorePlainBooks}>
              {plainBookLoading ? "加载中..." : "继续加载普通绘本"}
            </button>
          )}
          <div className="privacy-callout">投稿前必须确认作品不是定制绘本，且不包含儿童个人隐私；已经创建过投稿的绘本不会重复出现在这里。</div>
          <div className="modal-actions">
            <button className="button secondary" type="button" onClick={() => setOpen(null)}>取消</button>
            <button className="button primary" type="button" disabled={submitting || selectableBooks.length === 0} onClick={submitNew}>{submitting ? "创建中..." : "创建投稿草稿"}</button>
          </div>
        </Modal>
      )}
      {open === "privacy" && (
        <Modal title="投稿隐私确认" onClose={() => setOpen(null)}>
          {selectedSubmission && <p>当前投稿：<strong>{selectedSubmission.title}</strong></p>}
          <p>请确认投稿作品不含儿童姓名、家庭信息、照片或个别行为描述。</p>
          <div className="check-list">
            <label><input type="checkbox" defaultChecked />已检查正文</label>
            <label><input type="checkbox" defaultChecked />已检查插图描述</label>
            <label><input type="checkbox" defaultChecked />已确认园所授权</label>
          </div>
          <div className="modal-actions">
            <button className="button secondary" type="button" onClick={() => setOpen(null)}>取消</button>
            <button className="button primary" type="button" disabled={submitting || selectedSubmission?.privacyConfirmed} onClick={confirmPrivacy}>{submitting ? "保存中..." : selectedSubmission?.privacyConfirmed ? "已确认" : "确认无隐私风险"}</button>
          </div>
        </Modal>
      )}
    </div>
  );
}

function matchesSubmissionFilter(item: MarketplaceSubmission, statusFilter: string) {
  return !statusFilter || item.status === statusFilter;
}
