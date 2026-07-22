import { FormEvent, useEffect, useState } from "react";
import { useOutletContext } from "react-router-dom";
import { createMember, listClassroomsPage, listMembersPage, revokeMemberInvitation, shouldUseApi, type PaginationMeta } from "../../api/client";
import { Badge, Card, EmptyState, Modal, Notice, PageHeader } from "../../components/ui";
import type { Classroom, Workspace, WorkspaceMember } from "../../types/domain";
import { memberStatusLabel, roleLabel } from "../../utils/labels";

const PAGE_SIZE = 12;
const CLASSROOM_PAGE_SIZE = 50;

const members = [
  { name: "王老师", contact: "wang@example.com", role: "school_teacher", status: "active", classes: "小一班" },
  { name: "陈老师", contact: "chen@example.com", role: "school_teacher", status: "invited", classes: "中一班、小二班" },
  { name: "园长李老师", contact: "admin@example.com", role: "school_admin", status: "active", classes: "全部" },
];

export function MembersPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const [open, setOpen] = useState(false);
  const [notice, setNotice] = useState<{ title: string; copy: string } | null>(null);
  const [latestInviteUrl, setLatestInviteUrl] = useState("");
  const [remoteMembers, setRemoteMembers] = useState<WorkspaceMember[]>([]);
  const [offset, setOffset] = useState(0);
  const [pageMeta, setPageMeta] = useState<PaginationMeta | null>(null);
  const [classrooms, setClassrooms] = useState<Classroom[]>([]);
  const [classroomMeta, setClassroomMeta] = useState<PaginationMeta | null>(null);
  const [classroomLoading, setClassroomLoading] = useState(false);
  const [loading, setLoading] = useState(shouldUseApi);
  const [submitting, setSubmitting] = useState(false);
  const [revokingMemberId, setRevokingMemberId] = useState("");
  const [error, setError] = useState("");
  const [form, setForm] = useState({ name: "", email: "", classroom: "" });
  const rows: WorkspaceMember[] = shouldUseApi ? remoteMembers : members.map((member, index) => ({
    id: `mock-member-${index}`,
    workspaceId: workspace.id,
    name: member.name,
    email: member.contact,
    role: member.role as WorkspaceMember["role"],
    status: member.status as WorkspaceMember["status"],
    classes: member.classes.split("、"),
  }));
  const initialLoading = loading && (!shouldUseApi || remoteMembers.length === 0);

  useEffect(() => {
    setOffset(0);
  }, [workspace.id]);

  useEffect(() => {
    if (!shouldUseApi) return;
    let mounted = true;
    setLoading(true);
    setError("");
    if (offset === 0) {
      setRemoteMembers([]);
      setClassrooms([]);
      setPageMeta(null);
      setClassroomMeta(null);
    }
    Promise.all([
      listMembersPage(workspace.id, { limit: PAGE_SIZE, offset }),
      offset === 0 ? listClassroomsPage(workspace.id, { limit: CLASSROOM_PAGE_SIZE, offset: 0 }) : Promise.resolve(null),
    ])
      .then(([membersPage, classroomPage]) => {
        if (!mounted) return;
        setRemoteMembers((items) => (
          offset === 0
            ? membersPage.data
            : [...items, ...membersPage.data.filter((member) => !items.some((item) => item.id === member.id))]
        ));
        setPageMeta(membersPage.meta);
        if (classroomPage) {
          const classRows = classroomPage.data;
          setClassrooms(classRows);
          setClassroomMeta(classroomPage.meta);
          setForm((value) => ({ ...value, classroom: value.classroom || classRows[0]?.name || "" }));
        }
      })
      .catch((err: Error) => {
        if (!mounted) return;
        if (offset === 0) {
          setRemoteMembers([]);
          setClassrooms([]);
          setPageMeta(null);
          setClassroomMeta(null);
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
      setNotice({ title: "邀请已发送", copy: "这是 mock 反馈：真实接入后老师会收到邀请链接，接受后成为 school_teacher。" });
      return;
    }
    setSubmitting(true);
    setNotice(null);
    try {
      const member = await createMember(workspace.id, {
        name: form.name,
        email: form.email,
        classes: form.classroom ? [form.classroom] : [],
      });
      setRemoteMembers((items) => [member, ...items.filter((item) => item.id !== member.id)]);
      setPageMeta((meta) => meta ? { ...meta, total: meta.total + 1 } : meta);
      setOpen(false);
      setForm({ name: "", email: "", classroom: classrooms[0]?.name || "" });
      const invitePath = member.invitationUrl || (member.invitationToken ? `/invite/${member.invitationToken}` : "");
      const inviteUrl = invitePath ? absoluteLinkUrl(invitePath) : "";
      setLatestInviteUrl(inviteUrl);
      setNotice({
        title: "邀请已发送",
        copy: inviteUrl
          ? `${member.email} 已加入邀请列表，邀请链接：${inviteUrl}`
          : `${member.email} 已加入邀请列表，状态为待接受。`,
      });
    } catch (err) {
      setNotice({ title: "邀请失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setSubmitting(false);
    }
  };

  function copyLatestInviteUrl() {
    if (!latestInviteUrl) return;
    setNotice({ title: "邀请链接已准备复制", copy: latestInviteUrl });
    copyText(latestInviteUrl).catch(() => undefined);
  }

  function copyMemberInviteUrl(member: WorkspaceMember) {
    const invitePath = member.invitationUrl || (member.invitationToken ? `/invite/${member.invitationToken}` : "");
    if (!invitePath) return;
    const inviteUrl = absoluteLinkUrl(invitePath);
    setLatestInviteUrl(inviteUrl);
    setNotice({ title: "邀请链接已准备复制", copy: inviteUrl });
    copyText(inviteUrl).catch(() => undefined);
  }

  async function revokeInvitation(member: WorkspaceMember) {
    if (!shouldUseApi) {
      setNotice({ title: "邀请已撤回", copy: "这是 mock 反馈：真实接入后待接受邀请会被停用。" });
      return;
    }
    setRevokingMemberId(member.id);
    setNotice(null);
    try {
      const updated = await revokeMemberInvitation(workspace.id, member.id);
      setRemoteMembers((items) => items.map((item) => (item.id === updated.id ? updated : item)));
      setNotice({ title: "邀请已撤回", copy: `${updated.email} 的邀请链接已停用。` });
    } catch (err) {
      setNotice({ title: "撤回失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setRevokingMemberId("");
    }
  }

  const loadMoreClassrooms = async () => {
    if (!shouldUseApi || !classroomMeta?.has_more) return;
    setClassroomLoading(true);
    setNotice(null);
    try {
      const nextOffset = classroomMeta.offset + classroomMeta.limit;
      const page = await listClassroomsPage(workspace.id, { limit: CLASSROOM_PAGE_SIZE, offset: nextOffset });
      setClassrooms((items) => [
        ...items,
        ...page.data.filter((classroom) => !items.some((item) => item.id === classroom.id)),
      ]);
      setClassroomMeta(page.meta);
    } catch (err) {
      setNotice({ title: "班级选项加载失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setClassroomLoading(false);
    }
  };

  return (
  <div className="page-stack">
      <PageHeader eyebrow="成员管理" title="成员管理" copy="邀请老师、查看邀请状态、管理班级授权。" actions={<button className="button primary" type="button" onClick={() => setOpen(true)}>邀请老师</button>} />
      {notice && (
        <Notice
          title={notice.title}
          copy={notice.copy}
          tone={notice.title.includes("失败") ? "danger" : "good"}
          action={latestInviteUrl && !notice.title.includes("失败") ? (
            <button className="button secondary" type="button" onClick={copyLatestInviteUrl}>复制邀请链接</button>
          ) : undefined}
        />
      )}
      {initialLoading && <EmptyState title="正在加载成员" copy="正在读取园所成员和授权班级。" />}
      {error && rows.length === 0 && <EmptyState title="成员加载失败" copy={error} />}
      {error && rows.length > 0 && <Notice title="成员列表更新失败" copy={error} tone="danger" />}
      <Card>
        <div className="section-head">
          <div><p className="eyebrow">成员列表</p><h2>协作成员与授权范围</h2></div>
          {shouldUseApi && pageMeta?.has_more ? (
            <button className="button secondary" type="button" disabled={loading} onClick={() => setOffset((value) => value + PAGE_SIZE)}>
              {loading ? "加载中..." : "继续加载成员"}
            </button>
          ) : (
            <Badge tone="info">{shouldUseApi && pageMeta ? `${rows.length}/${pageMeta.total}` : rows.length} 位成员</Badge>
          )}
        </div>
        <div className="table-list">
          {rows.map((member) => (
            <div className="table-row" key={member.id}>
              <div><strong>{member.name}</strong><span>{member.email}</span></div>
              <span>{roleLabel[member.role]}</span>
              <span>{member.classes.length ? member.classes.join("、") : "未授权班级"}</span>
              <div className="inline-actions">
                <Badge tone={member.status === "active" ? "good" : "warn"}>{memberStatusLabel[member.status] || member.status}</Badge>
                {member.status === "invited" && (member.invitationUrl || member.invitationToken) && (
                  <>
                    <button className="button secondary" type="button" onClick={() => copyMemberInviteUrl(member)}>复制成员邀请链接</button>
                    <button className="button secondary" type="button" disabled={revokingMemberId === member.id} onClick={() => revokeInvitation(member)}>
                      {revokingMemberId === member.id ? "撤回中..." : "撤回邀请"}
                    </button>
                  </>
                )}
              </div>
            </div>
          ))}
        </div>
      </Card>
      {open && (
        <Modal title="邀请老师" onClose={() => setOpen(false)}>
          <form onSubmit={submit}>
            <label>老师姓名<input value={form.name} onChange={(event) => setForm({ ...form, name: event.target.value })} placeholder="例如：陈老师" /></label>
            <label>老师邮箱<input required type="email" value={form.email} onChange={(event) => setForm({ ...form, email: event.target.value })} placeholder="teacher@example.com" /></label>
            <label>授权班级<select value={form.classroom} onChange={(event) => setForm({ ...form, classroom: event.target.value })}><option value="">暂不授权班级</option>{classrooms.map((item) => <option key={item.id} value={item.name}>{item.name}</option>)}</select></label>
            {shouldUseApi && classroomMeta?.has_more && (
              <button className="button secondary" type="button" disabled={classroomLoading} onClick={loadMoreClassrooms}>
                {classroomLoading ? "加载中..." : "继续加载班级选项"}
              </button>
            )}
            <div className="modal-actions">
              <button className="button secondary" type="button" onClick={() => setOpen(false)}>取消</button>
              <button className="button primary" type="submit" disabled={submitting}>{submitting ? "发送中..." : "发送邀请"}</button>
            </div>
          </form>
        </Modal>
      )}
    </div>
  );
}

function absoluteLinkUrl(path: string) {
  if (/^https?:\/\//i.test(path)) return path;
  return `${window.location.origin}${path.startsWith("/") ? path : `/${path}`}`;
}

async function copyText(value: string) {
  if (navigator.clipboard?.writeText) {
    try {
      await Promise.race([
        navigator.clipboard.writeText(value),
        new Promise((_, reject) => window.setTimeout(() => reject(new Error("clipboard timeout")), 300)),
      ]);
      return;
    } catch {
      // Continue with the textarea fallback for browsers that expose clipboard but block it.
    }
  }
  const textArea = document.createElement("textarea");
  textArea.value = value;
  textArea.setAttribute("readonly", "true");
  textArea.style.position = "fixed";
  textArea.style.opacity = "0";
  document.body.appendChild(textArea);
  textArea.focus();
  textArea.select();
  const copied = document.execCommand("copy");
  document.body.removeChild(textArea);
  if (!copied) {
    throw new Error("浏览器没有允许复制，请手动复制邀请链接。");
  }
}
