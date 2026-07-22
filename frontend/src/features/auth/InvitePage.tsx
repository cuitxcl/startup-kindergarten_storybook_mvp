import { CheckCircle2 } from "lucide-react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { useEffect, useState } from "react";
import { acceptInvitation, getInvitation, shouldUseApi } from "../../api/client";
import { invitations } from "../../data/mock";
import { Badge, Card, EmptyState, Notice } from "../../components/ui";
import type { WorkspaceInvitation } from "../../types/domain";

function statusLabel(status: WorkspaceInvitation["status"]) {
  const labels: Record<WorkspaceInvitation["status"], string> = {
    pending: "待接受",
    invited: "待接受",
    accepted: "已接受",
    active: "已加入",
    expired: "已过期",
    revoked: "已撤回",
  };
  return labels[status];
}

export function InvitePage() {
  const { token } = useParams();
  const navigate = useNavigate();
  const mockInvite = invitations[0];
  const [invite, setInvite] = useState<WorkspaceInvitation | null>(shouldUseApi ? null : mockInvite);
  const [loading, setLoading] = useState(shouldUseApi);
  const [accepting, setAccepting] = useState(false);
  const [notice, setNotice] = useState<{ title: string; copy: string; tone: "good" | "danger" } | null>(null);

  useEffect(() => {
    if (!token) {
      setInvite(null);
      setLoading(false);
      return;
    }
    if (!shouldUseApi) {
      setInvite(mockInvite);
      setLoading(false);
      return;
    }
    setLoading(true);
    setNotice(null);
    getInvitation(token)
      .then(setInvite)
      .catch((err) => {
        setInvite(null);
        setNotice({ title: "邀请不可用", copy: err instanceof Error ? err.message : "请联系园所管理员重新邀请。", tone: "danger" });
      })
      .finally(() => setLoading(false));
  }, [mockInvite, token]);

  async function accept() {
    if (!token) return;
    if (!shouldUseApi) {
      navigate("/app/school-1/dashboard");
      return;
    }
    setAccepting(true);
    setNotice(null);
    try {
      const accepted = await acceptInvitation(token);
      setInvite(accepted);
      setNotice({ title: "已接受邀请", copy: `你已加入 ${accepted.workspaceName}，角色为老师。`, tone: "good" });
      navigate(`/app/${accepted.workspaceId}/dashboard`);
    } catch (err) {
      setNotice({ title: "接受失败", copy: err instanceof Error ? err.message : "请稍后重试。", tone: "danger" });
    } finally {
      setAccepting(false);
    }
  }

  if (loading) {
    return <main className="auth-page"><EmptyState title="正在读取邀请" copy="正在确认园所邀请信息。" /></main>;
  }

  if (!invite) {
    return <main className="auth-page"><EmptyState title="邀请不可用" copy="没有找到这条邀请，请联系园所管理员重新发送。" action={<Link className="button secondary" to="/login">返回登录</Link>} /></main>;
  }

  return (
    <main className="auth-page">
      <Card className="invite-card">
        <p className="eyebrow">老师邀请</p>
        <h1>加入 {invite.workspaceName}</h1>
        <p>{invite.invitedBy} 邀请你成为园所空间里的老师。接受后，你仍然保留自己的个人空间。</p>
        {notice && <Notice title={notice.title} copy={notice.copy} tone={notice.tone} />}
        <div className="review-list">
          <div><span>获得角色</span><strong>老师</strong></div>
          <div><span>邀请账号</span><strong>{invite.invitedContact || "当前登录账号"}</strong></div>
          <div><span>可访问班级</span><strong>{invite.classrooms.length ? invite.classrooms.join("、") : "由园所后续授权"}</strong></div>
          <div><span>邀请状态</span><Badge tone={invite.status === "invited" || invite.status === "pending" ? "warn" : "good"}>{statusLabel(invite.status)}</Badge></div>
        </div>
        <div className="privacy-callout">
          <CheckCircle2 size={18} />
          个人空间内容不会自动共享给园所。
        </div>
        <button className="button primary" type="button" disabled={accepting || invite.status === "active"} onClick={accept}>
          {accepting ? "正在接受..." : invite.status === "active" ? "已加入园所" : "接受邀请并进入园所空间"}
        </button>
        <Link className="button secondary" to="/login">换账号登录</Link>
      </Card>
    </main>
  );
}
