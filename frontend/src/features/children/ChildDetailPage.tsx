import { FormEvent, useEffect, useState } from "react";
import { Link, useOutletContext, useParams } from "react-router-dom";
import { archiveChild, getChild, listStorybooksPage, restoreChild, updateChild } from "../../api/client";
import { Badge, Card, Modal, Notice, PageHeader } from "../../components/ui";
import type { ChildProfile, Storybook, Workspace } from "../../types/domain";

function splitTags(value: string) {
  return value
    .split(/[、,，]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

export function ChildDetailPage() {
  const { childId } = useParams();
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const [open, setOpen] = useState(false);
  const [notice, setNotice] = useState<{ title: string; copy: string; tone?: "good" | "info" } | null>(null);
  const [remoteChild, setRemoteChild] = useState<ChildProfile | null>(null);
  const [remoteRelatedBooks, setRemoteRelatedBooks] = useState<Storybook[]>([]);
  const [remoteSourceBooks, setRemoteSourceBooks] = useState<Storybook[]>([]);
  const [loading, setLoading] = useState(true);
  const [archiving, setArchiving] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [error, setError] = useState("");
  const child = remoteChild;
  const related = remoteRelatedBooks;
  const preferredSource = remoteSourceBooks
    .filter((item) => item.workspaceId === workspace.id && item.type === "plain")
    .find((item) => item.status === "exportable")
    || remoteSourceBooks.find((item) => item.workspaceId === workspace.id && item.type === "plain");
  const customizeTarget = preferredSource ? `/app/${workspace.id}/storybooks/${preferredSource.id}/customize?childId=${child?.id || ""}` : `/app/${workspace.id}/storybooks`;
  const customizeActionLabel = preferredSource ? `从《${preferredSource.title}》生成定制绘本` : "先选择普通绘本";
  const [form, setForm] = useState({
    nickname: "",
    ageGroup: "3-4 岁",
    focus: "",
    interests: "",
    traits: "",
  });

  useEffect(() => {
    if (!childId) return;
    let mounted = true;
    setLoading(true);
    setRemoteChild(null);
    setRemoteRelatedBooks([]);
    setRemoteSourceBooks([]);
    setError("");
    async function load() {
      try {
        const [childItem, relatedBooks, sourceBooks] = await Promise.all([
          getChild(workspace.id, childId!),
          listStorybooksPage(workspace.id, { type: "custom", targetChildId: childId!, limit: 6 }),
          listStorybooksPage(workspace.id, { type: "plain", limit: 12 }),
        ]);
        if (!mounted) return;
        setRemoteChild(childItem);
        setRemoteRelatedBooks(relatedBooks.data);
        setRemoteSourceBooks(sourceBooks.data);
        setForm({
          nickname: childItem.nickname,
          ageGroup: childItem.ageGroup,
          focus: childItem.focus,
          interests: childItem.interests.join("、"),
          traits: childItem.traits.join("、"),
        });
        setError("");
      } catch (err) {
        if (!mounted) return;
        setRemoteChild(null);
        setRemoteRelatedBooks([]);
        setRemoteSourceBooks([]);
        setError(err instanceof Error ? err.message : "无法读取儿童资料");
      } finally {
        if (mounted) setLoading(false);
      }
    }
    load();
    return () => {
      mounted = false;
    };
  }, [childId, workspace.id]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!child) return;
    try {
      const updated = await updateChild(workspace.id, child.id, {
        nickname: form.nickname,
        ageGroup: form.ageGroup,
        focus: form.focus,
        interests: splitTags(form.interests),
        traits: splitTags(form.traits),
      });
      setRemoteChild(updated);
      setOpen(false);
      setNotice({ title: "儿童资料已保存", copy: `${updated.nickname} 的资料已写入后端。`, tone: "good" });
    } catch (err) {
      setNotice({ title: "保存失败", copy: err instanceof Error ? err.message : "请稍后重试。", tone: "info" });
    }
  }

  async function archiveCurrentChild() {
    if (!child) return;
    try {
      setArchiving(true);
      const archived = await archiveChild(workspace.id, child.id);
      setRemoteChild(archived);
      setNotice({ title: "儿童资料已归档", copy: `${archived.nickname} 将不再出现在定制绘本选择列表，历史绘本仍可查看。`, tone: "good" });
    } catch (err) {
      setNotice({ title: "归档失败", copy: err instanceof Error ? err.message : "请稍后重试。", tone: "info" });
    } finally {
      setArchiving(false);
    }
  }

  async function restoreCurrentChild() {
    if (!child) return;
    try {
      setRestoring(true);
      const restored = await restoreChild(workspace.id, child.id);
      setRemoteChild(restored);
      setNotice({ title: "儿童资料已恢复", copy: `${restored.nickname} 已重新回到可定制儿童列表。`, tone: "good" });
    } catch (err) {
      setNotice({ title: "恢复失败", copy: err instanceof Error ? err.message : "请稍后重试。", tone: "info" });
    } finally {
      setRestoring(false);
    }
  }

  if (loading) {
    return <div className="page-stack"><Notice title="正在读取儿童资料" copy="正在从后端加载儿童档案。" tone="info" /></div>;
  }

  if (error || !child) {
    return <div className="page-stack"><Notice title="儿童资料加载失败" copy={error || "当前儿童不存在"} tone="info" /></div>;
  }

  return (
    <div className="page-stack">
      <PageHeader
        eyebrow="儿童档案"
        title={child.nickname}
        copy={`${child.ageGroup} · ${child.focus}`}
        actions={
          <>
            <Link className="button secondary" to={`/app/${workspace.id}/storybooks`}>回到绘本列表</Link>
            {child.status !== "archived" && <Link className="button primary" to={customizeTarget}>{customizeActionLabel}</Link>}
            <button className="button secondary" type="button" onClick={() => setOpen(true)}>编辑资料</button>
            {child.status === "archived" ? (
              <button className="button primary" type="button" disabled={restoring} onClick={restoreCurrentChild}>{restoring ? "恢复中" : "恢复资料"}</button>
            ) : (
              <button className="button secondary" type="button" disabled={archiving} onClick={archiveCurrentChild}>{archiving ? "归档中" : "归档资料"}</button>
            )}
          </>
        }
      />
      {notice && <Notice title={notice.title} copy={notice.copy} tone={notice.tone || "good"} />}
      <section className="two-column">
        <Card>
          <h2>个性化元素</h2>
          <div className="tag-list">{child.interests.map((tag) => <Badge key={tag} tone="info">{tag}</Badge>)}</div>
          <h2>性格特点</h2>
          <div className="tag-list">{child.traits.map((tag) => <Badge key={tag}>{tag}</Badge>)}</div>
          <p>{child.focus}</p>
        </Card>
        <Card>
          <h2>相关定制绘本</h2>
          {related.length === 0 ? (
            <>
              <p>还没有为这个孩子生成定制绘本。可以先从一本文字和插图都稳定的普通绘本开始。</p>
              <Link className="button primary" to={customizeTarget}>{customizeActionLabel}</Link>
            </>
          ) : (
            <div className="compact-list">
              {related.map((book) => (
                <Link className="compact-row" to={`/app/${workspace.id}/storybooks/${book.id}`} key={book.id}>
                  <div>
                    <strong>{book.title}</strong>
                    <span>{book.updatedAt} · {book.status === "exportable" ? "可导出" : "继续编辑"}</span>
                  </div>
                  <Badge tone="good">查看</Badge>
                </Link>
              ))}
            </div>
          )}
        </Card>
      </section>
      {open && (
        <Modal title={`编辑 ${child.nickname} 的资料`} onClose={() => setOpen(false)}>
          <form onSubmit={submit}>
            <label>孩子称呼<input value={form.nickname} onChange={(event) => setForm((current) => ({ ...current, nickname: event.target.value }))} /></label>
            <label>年龄段<select value={form.ageGroup} onChange={(event) => setForm((current) => ({ ...current, ageGroup: event.target.value }))}><option>3-4 岁</option><option>4-5 岁</option><option>5-6 岁</option></select></label>
            <label>关注点<textarea rows={3} value={form.focus} onChange={(event) => setForm((current) => ({ ...current, focus: event.target.value }))} /></label>
            <label>兴趣标签<input value={form.interests} onChange={(event) => setForm((current) => ({ ...current, interests: event.target.value }))} /></label>
            <label>性格特点<input value={form.traits} onChange={(event) => setForm((current) => ({ ...current, traits: event.target.value }))} /></label>
            <div className="modal-actions">
              <button className="button secondary" type="button" onClick={() => setOpen(false)}>取消</button>
              <button className="button primary" type="submit">保存资料</button>
            </div>
          </form>
        </Modal>
      )}
    </div>
  );
}
