import { FormEvent, useEffect, useState } from "react";
import { useOutletContext } from "react-router-dom";
import { createMember, listClassroomsPage, listMembersPage, revokeMemberInvitation, type PaginationMeta } from "../../api/client";
import { Badge, Card, EmptyState, Modal, Notice, PageHeader } from "../../components/ui";
import type { Classroom, Workspace, WorkspaceMember } from "../../types/domain";
import { absoluteAppUrl, copyText } from "../../utils/clipboard";
import { memberStatusLabel, roleLabel } from "../../utils/labels";

const PAGE_SIZE = 12;
const CLASSROOM_PAGE_SIZE = 50;

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
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [revokingMemberId, setRevokingMemberId] = useState("");
  const [error, setError] = useState("");
  const [form, setForm] = useState({ name: "", email: "", classroom: "" });
  const rows: WorkspaceMember[] = remoteMembers;
  const initialLoading = loading && remoteMembers.length === 0;

  useEffect(() => {
    setOffset(0);
  }, [workspace.id]);

  useEffect(() => {
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
      const inviteUrl = invitePath ? absoluteAppUrl(invitePath) : "";
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
    const inviteUrl = absoluteAppUrl(invitePath);
    setLatestInviteUrl(inviteUrl);
    setNotice({ title: "邀请链接已准备复制", copy: inviteUrl });
    copyText(inviteUrl).catch(() => undefined);
  }

  async function revokeInvitation(member: WorkspaceMember) {
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
    if (!classroomMeta?.has_more) return;
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
      <PageHeader title="成员管理" copy="邀请老师、查看邀请状态、管理班级授权。" actions={<button className="button primary" type="button" onClick={() => setOpen(true)}>邀请老师</button>} />
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
          {pageMeta?.has_more ? (
            <button className="button secondary" type="button" disabled={loading} onClick={() => setOffset((value) => value + PAGE_SIZE)}>
              {loading ? "加载中..." : "继续加载成员"}
            </button>
          ) : (
            <Badge tone="info">{pageMeta ? `${rows.length}/${pageMeta.total}` : rows.length} 位成员</Badge>
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
                  <details className="row-actions">
                    <summary>邀请操作</summary>
                    <div className="inline-actions">
                      <button className="button secondary" type="button" onClick={() => copyMemberInviteUrl(member)}>复制链接</button>
                      <button className="button secondary" type="button" disabled={revokingMemberId === member.id} onClick={() => revokeInvitation(member)}>
                        {revokingMemberId === member.id ? "撤回中..." : "撤回邀请"}
                      </button>
                    </div>
                  </details>
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
            {classroomMeta?.has_more && (
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
