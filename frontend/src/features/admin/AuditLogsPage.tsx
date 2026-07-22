import { useEffect, useState } from "react";
import { useOutletContext } from "react-router-dom";
import { listAuditLogsPage, shouldUseApi, type PaginationMeta } from "../../api/client";
import { Badge, Card, EmptyState, Notice, PageHeader } from "../../components/ui";
import type { AuditLog, Workspace } from "../../types/domain";

const PAGE_SIZE = 20;

const mockLogs: AuditLog[] = [
  {
    id: "audit-1",
    workspaceId: "prototype-school-space",
    actorName: "林老师",
    action: "marketplace_submission.privacy_confirmed",
    resourceType: "marketplace_submission",
    resourceId: "submission-1",
    metadata: { status: "submitted", privacy_confirmed: true },
    createdAt: "刚刚",
  },
  {
    id: "audit-2",
    workspaceId: "prototype-school-space",
    actorName: "林老师",
    action: "storybook.share_link_created",
    resourceType: "share_link",
    resourceId: "share-1",
    metadata: { status: "active" },
    createdAt: "10 分钟前",
  },
];

const actionLabel: Record<string, string> = {
  "child.created": "创建儿童档案",
  "child.updated": "更新儿童档案",
  "storybook.created": "创建绘本",
  "storybook.updated": "更新绘本设置",
  "storybook.page_updated": "更新绘本分页",
  "storybook.role_updated": "更新角色设定",
  "storybook.custom_derived": "生成定制绘本",
  "parent_intake.submitted": "提交家长资料",
  "parent_intake.confirmed": "确认家长资料",
  "parent_intake_link.created": "创建家长资料链接",
  "parent_intake_link.revoked": "撤回家长资料链接",
  "marketplace_template.copied": "复制市场模板",
  "generation_job.created": "创建生成任务",
  "generation_job.retried": "重试生成任务",
  "generation_job.canceled": "取消生成任务",
  "generation_job.recovered": "恢复生成队列",
  "workspace_member.invited": "邀请老师",
  "classroom.created": "创建班级",
  "storybook.export_created": "创建导出",
  "storybook.share_link_created": "创建分享链接",
  "storybook.share_link_revoked": "撤回分享链接",
  "marketplace_submission.created": "创建投稿",
  "marketplace_submission.privacy_blocked": "隐私风险拦截",
  "marketplace_submission.privacy_confirmed": "隐私确认",
  "marketplace_submission.approved": "审核上架",
  "share_link.public_export_created": "公开链接导出",
};

const resourceLabel: Record<string, string> = {
  workspace_member: "成员",
  classroom: "班级",
  child: "儿童档案",
  storybook: "绘本",
  storybook_page: "绘本分页",
  storybook_role: "角色设定",
  parent_intake: "家长资料",
  parent_intake_link: "家长资料链接",
  generation_job: "生成任务",
  export_job: "导出任务",
  share_link: "分享链接",
  marketplace_submission: "市场投稿",
};

const metadataStatusLabel: Record<string, string> = {
  submitted: "待审核",
  active: "已启用",
  listed: "已上架",
  draft: "草稿",
  approved: "已通过",
  rejected: "已驳回",
  revoked: "已撤回",
  archived: "已归档",
  queued: "排队中",
  running: "正在生成",
  succeeded: "已完成",
  failed: "生成失败",
  canceled: "已取消",
};

function metadataSummary(metadata: Record<string, unknown>) {
  const entries = Object.entries(metadata).filter(([, value]) => value !== null && value !== undefined);
  if (!entries.length) return "无额外摘要";
  return entries
    .slice(0, 3)
    .map(([key, value]) => `${key}: ${Array.isArray(value) ? value.join("、") : metadataStatusLabel[String(value)] || String(value)}`)
    .join("；");
}

export function AuditLogsPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const [logs, setLogs] = useState<AuditLog[]>(shouldUseApi ? [] : mockLogs);
  const [offset, setOffset] = useState(0);
  const [pageMeta, setPageMeta] = useState<PaginationMeta | null>(null);
  const [loading, setLoading] = useState(shouldUseApi);
  const [error, setError] = useState("");
  const initialLoading = loading && (!shouldUseApi || logs.length === 0);

  useEffect(() => {
    setOffset(0);
  }, [workspace.id]);

  useEffect(() => {
    if (!shouldUseApi) return;
    let mounted = true;
    setLoading(true);
    if (offset === 0) {
      setLogs([]);
      setPageMeta(null);
    }
    setError("");
    listAuditLogsPage(workspace.id, { limit: PAGE_SIZE, offset })
      .then((page) => {
        if (!mounted) return;
        setLogs((current) => (
          offset === 0
            ? page.data
            : [...current, ...page.data.filter((log) => !current.some((item) => item.id === log.id))]
        ));
        setPageMeta(page.meta);
      })
      .catch((err: Error) => {
        if (!mounted) return;
        if (offset === 0) {
          setLogs([]);
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

  return (
    <div className="page-stack">
      <PageHeader eyebrow="审计日志" title="最近操作记录" copy="查看当前园所空间内的邀请、班级、导出、分享和投稿关键操作。" />
      {initialLoading ? (
        <EmptyState title="正在加载审计日志" copy="正在读取当前园所操作记录。" />
      ) : error && logs.length === 0 ? (
        <EmptyState title="审计日志加载失败" copy={error} />
      ) : logs.length === 0 ? (
        <EmptyState title="暂无审计记录" copy="完成邀请、分享、导出或投稿后，这里会出现可追溯记录。" />
      ) : (
        <>
          {error && <Notice title="审计日志更新失败" copy={error} tone="danger" />}
          <Card>
            <div className="section-head">
              <div>
                <p className="eyebrow">操作追踪</p>
                <h2>已显示 {logs.length}{shouldUseApi && pageMeta ? ` / 共 ${pageMeta.total}` : ""} 条记录</h2>
              </div>
              {shouldUseApi && pageMeta?.has_more ? (
                <button className="button secondary" type="button" disabled={loading} onClick={() => setOffset((value) => value + PAGE_SIZE)}>
                  {loading ? "加载中..." : "继续加载日志"}
                </button>
              ) : (
                <Badge tone="info">{logs.length} 条记录</Badge>
              )}
            </div>
            <div className="table-list">
              {logs.map((log) => (
                <div className="table-row audit-row" key={log.id}>
                  <div>
                    <strong>{actionLabel[log.action] || log.action}</strong>
                    <span>{log.actorName || "公开访问者"} · {log.createdAt}</span>
                  </div>
                  <span>{resourceLabel[log.resourceType] || log.resourceType}</span>
                  <span>{metadataSummary(log.metadata)}</span>
                  <Badge tone={log.action.includes("privacy") || log.action.includes("approved") ? "warn" : "neutral"}>{log.resourceId ? "已关联资源" : "系统记录"}</Badge>
                </div>
              ))}
            </div>
          </Card>
        </>
      )}
    </div>
  );
}
