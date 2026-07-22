import { FormEvent, useEffect, useState } from "react";
import { useOutletContext } from "react-router-dom";
import { archiveClassroom, createClassroom, listClassroomsPage, shouldUseApi, type PaginationMeta } from "../../api/client";
import { Badge, Card, EmptyState, Modal, Notice, PageHeader } from "../../components/ui";
import type { Classroom, Workspace } from "../../types/domain";
import { classroomStatusLabel } from "../../utils/labels";

const PAGE_SIZE = 12;

const classes = [
  { name: "小一班", age: "3-4 岁", teachers: 2, children: 18, status: "active" },
  { name: "中一班", age: "4-5 岁", teachers: 2, children: 21, status: "active" },
  { name: "大二班", age: "5-6 岁", teachers: 1, children: 19, status: "archived" },
];

export function ClassesPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const [open, setOpen] = useState(false);
  const [notice, setNotice] = useState<{ title: string; copy: string } | null>(null);
  const [remoteClasses, setRemoteClasses] = useState<Classroom[]>([]);
  const [offset, setOffset] = useState(0);
  const [pageMeta, setPageMeta] = useState<PaginationMeta | null>(null);
  const [loading, setLoading] = useState(shouldUseApi);
  const [submitting, setSubmitting] = useState(false);
  const [archivingId, setArchivingId] = useState("");
  const [error, setError] = useState("");
  const [form, setForm] = useState({ name: "", ageGroup: "3-4 岁" });
  const rows = shouldUseApi ? remoteClasses : classes.map((item, index) => ({
    id: `mock-class-${index}`,
    workspaceId: workspace.id,
    name: item.name,
    ageGroup: item.age,
    teachers: item.teachers,
    children: item.children,
    status: item.status as Classroom["status"],
  }));
  const initialLoading = loading && (!shouldUseApi || remoteClasses.length === 0);

  useEffect(() => {
    setOffset(0);
  }, [workspace.id]);

  useEffect(() => {
    if (!shouldUseApi) return;
    let mounted = true;
    setLoading(true);
    setError("");
    if (offset === 0) {
      setRemoteClasses([]);
      setPageMeta(null);
    }
    listClassroomsPage(workspace.id, { limit: PAGE_SIZE, offset })
      .then((classroomsPage) => {
        if (!mounted) return;
        setRemoteClasses((items) => (
          offset === 0
            ? classroomsPage.data
            : [...items, ...classroomsPage.data.filter((item) => !items.some((row) => row.id === item.id))]
        ));
        setPageMeta(classroomsPage.meta);
      })
      .catch((err: Error) => {
        if (!mounted) return;
        if (offset === 0) {
          setRemoteClasses([]);
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
  }, [offset, workspace.id]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!shouldUseApi) {
      setOpen(false);
      setNotice({ title: "班级已创建", copy: "这是 mock 反馈：真实接入后可继续邀请老师并导入儿童档案。" });
      return;
    }
    setSubmitting(true);
    setNotice(null);
    try {
      const classroom = await createClassroom(workspace.id, form);
      setRemoteClasses((items) => [classroom, ...items.filter((item) => item.id !== classroom.id)]);
      setPageMeta((meta) => meta ? { ...meta, total: meta.total + 1 } : meta);
      setOpen(false);
      setForm({ name: "", ageGroup: "3-4 岁" });
      setNotice({ title: "班级已创建", copy: `${classroom.name} 已加入当前园所空间。` });
    } catch (err) {
      setNotice({ title: "创建失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setSubmitting(false);
    }
  };

  async function archive(item: Classroom) {
    if (!shouldUseApi) {
      setNotice({ title: "班级已归档", copy: "这是 mock 反馈：真实接入后班级会从授权选项中移除。" });
      return;
    }
    setArchivingId(item.id);
    setNotice(null);
    try {
      const archived = await archiveClassroom(workspace.id, item.id);
      setRemoteClasses((items) => items.filter((row) => row.id !== archived.id));
      setPageMeta((meta) => meta ? { ...meta, total: Math.max(0, meta.total - 1) } : meta);
      setNotice({ title: "班级已归档", copy: `${archived.name} 已从当前使用班级中移除。` });
    } catch (err) {
      setNotice({ title: "归档失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setArchivingId("");
    }
  }

  return (
    <div className="page-stack">
      <PageHeader eyebrow="班级管理" title="班级管理" copy="班级是老师授权和儿童档案归属的核心边界。" actions={<button className="button primary" type="button" onClick={() => setOpen(true)}>创建班级</button>} />
      {notice && <Notice title={notice.title} copy={notice.copy} tone={notice.title.includes("失败") ? "danger" : "good"} />}
      {initialLoading && <EmptyState title="正在加载班级" copy="正在读取园所班级列表。" />}
      {error && rows.length === 0 && <EmptyState title="班级加载失败" copy={error} />}
      {error && rows.length > 0 && <Notice title="班级列表更新失败" copy={error} tone="danger" />}
      <Card>
        <div className="section-head">
          <div><p className="eyebrow">班级列表</p><h2>班级资料与儿童数量</h2></div>
          {shouldUseApi && pageMeta?.has_more ? (
            <button className="button secondary" type="button" disabled={loading} onClick={() => setOffset((value) => value + PAGE_SIZE)}>
              {loading ? "加载中..." : "继续加载班级"}
            </button>
          ) : (
            <Badge tone="info">{shouldUseApi && pageMeta ? `${rows.length}/${pageMeta.total}` : rows.length} 个班级</Badge>
          )}
        </div>
        <div className="table-list">
          {rows.map((item) => (
              <div className="table-row" key={item.id}>
              <div><strong>{item.name}</strong><span>{item.ageGroup}</span></div>
              <span>{item.teachers} 位老师</span>
              <span>{item.children} 名儿童</span>
              <div className="inline-actions">
                <Badge tone={item.status === "active" ? "good" : "neutral"}>{classroomStatusLabel[item.status] || item.status}</Badge>
                {item.status === "active" && item.children === 0 ? (
                  <button className="button secondary" type="button" disabled={archivingId === item.id} onClick={() => archive(item)}>
                    {archivingId === item.id ? "归档中..." : "归档班级"}
                  </button>
                ) : item.status === "active" ? (
                  <span>有儿童档案，暂不可归档</span>
                ) : null}
              </div>
            </div>
          ))}
        </div>
      </Card>
      {open && (
        <Modal title="创建班级" onClose={() => setOpen(false)}>
          <form onSubmit={submit}>
            <label>班级名称<input required value={form.name} onChange={(event) => setForm({ ...form, name: event.target.value })} placeholder="例如：小二班" /></label>
            <label>年龄段<select value={form.ageGroup} onChange={(event) => setForm({ ...form, ageGroup: event.target.value })}><option>3-4 岁</option><option>4-5 岁</option><option>5-6 岁</option></select></label>
            <div className="modal-actions">
              <button className="button secondary" type="button" onClick={() => setOpen(false)}>取消</button>
              <button className="button primary" type="submit" disabled={submitting}>{submitting ? "创建中..." : "确认创建"}</button>
            </div>
          </form>
        </Modal>
      )}
    </div>
  );
}
