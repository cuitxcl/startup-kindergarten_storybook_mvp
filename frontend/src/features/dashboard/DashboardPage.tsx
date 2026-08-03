import {
  ArrowRight,
  BookOpen,
  ClipboardCheck,
  Clock3,
  FileCheck2,
  Library,
  ShieldCheck,
  Sparkles,
  UserPlus,
  UsersRound,
} from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useOutletContext } from "react-router-dom";
import {
  dashboard,
  getUserStorageQuota,
  getWorkspaceStorageQuota,
  listGenerationJobsPage,
  type DashboardData,
  type GenerationJob,
  type UserStorageQuota,
  type WorkspaceStorageQuota,
} from "../../api/client";
import { Badge, Card, EmptyState, PageHeader, statusTone } from "../../components/ui";
import type { ChildProfile, MarketplaceSubmission, Storybook, Workspace } from "../../types/domain";
import {
  generationJobNextAction,
  generationJobStatusLabel,
  generationJobTypeLabel,
  storybookNextAction,
  storybookStatusLabel,
} from "../../utils/labels";

type DashboardTask = {
  title: string;
  copy: string;
  to: string;
  badge: string;
  tone?: "neutral" | "good" | "warn" | "danger" | "info";
};

type DashboardMetric = {
  label: string;
  value: number | string;
  copy: string;
  tone?: "neutral" | "good" | "warn" | "danger" | "info";
};

function defaultStorageQuota(workspace: Workspace): WorkspaceStorageQuota {
  const quotaBytes = workspace.type === "school" ? 5_368_709_120 : 209_715_200;
  return {
    workspaceId: workspace.id,
    workspaceType: workspace.type,
    quotaBytes,
    usedBytes: 0,
    remainingBytes: quotaBytes,
    usedPercent: 0,
    warningPercent: 80,
    warning: false,
    exceeded: false,
  };
}

function defaultUserStorageQuota(): UserStorageQuota {
  const quotaBytes = 209_715_200;
  return {
    userId: "local-dashboard-preview",
    quotaBytes,
    usedBytes: 0,
    remainingBytes: quotaBytes,
    usedPercent: 0,
    warningPercent: 80,
    warning: false,
    exceeded: false,
    personalWorkspaceCount: 1,
  };
}

function generationJobTitle(job: GenerationJob) {
  return generationJobTypeLabel[job.jobType] || job.jobType;
}

function generationJobCopy(job: GenerationJob) {
  if (job.status === "failed") {
    return generationJobNextAction(job);
  }
  if (job.status === "queued") return "任务已经进入队列，稍后会开始处理。";
  if (job.status === "running") return "任务正在执行，完成后会更新对应绘本内容。";
  if (job.status === "canceled") return "任务已取消，不会继续执行。";
  if (job.storybookId) return "已写入关联绘本，可进入详情继续编辑或导出。";
  return "已生成结构化结果，可继续推进下一步。";
}

function generationJobTime(job: GenerationJob) {
  return job.finishedAt || job.createdAt;
}

function getPrimary(workspace: Workspace, books: Storybook[], childItems: ChildProfile[]) {
  const editable = books.find((book) => book.status !== "exportable" && book.status !== "listed");
  const exportable = books.find((book) => book.status === "exportable");
  const incompleteChild = childItems.find((child) => child.completeness < 85);

  if (workspace.role === "school_admin") {
    return {
      icon: <ShieldCheck size={20} />,
      title: "先处理园所协作与投稿风险",
      copy: "优先确认投稿隐私、老师协作和班级资料完整度，再推进新的绘本生产。",
      action: { label: "查看园所投稿", to: "../admin/submissions" },
      meta: "管理员重点",
    };
  }

  if (workspace.role === "school_teacher") {
    return {
      icon: <BookOpen size={20} />,
      title: exportable ? `继续使用《${exportable.title}》` : "为授权班级准备一套绘本",
      copy: incompleteChild
        ? `${incompleteChild.nickname} 的资料完整度 ${incompleteChild.completeness}%，补齐后更适合生成定制绘本。`
        : "可以从普通绘本开始，也可以基于已有绘本为班级儿童生成定制副本。",
      action: { label: exportable ? "打开最近绘本" : "新建普通绘本", to: exportable ? `../storybooks/${exportable.id}` : "../storybooks/new" },
      meta: "老师今日重点",
    };
  }

  return {
    icon: <Sparkles size={20} />,
    title: editable ? `继续完成《${editable.title}》` : "生成或复用一套普通绘本",
    copy: editable
      ? storybookNextAction(editable)
      : "先做普通绘本，再按孩子资料生成定制副本，适合验证完整创作流程。",
    action: { label: editable ? "继续编辑" : "新建普通绘本", to: editable ? `../storybooks/${editable.id}` : "../storybooks/new" },
    meta: "个人空间重点",
  };
}

function getTasks(workspace: Workspace, books: Storybook[], childItems: ChildProfile[], submissionItems: MarketplaceSubmission[]): DashboardTask[] {
  const editable = books.filter((book) => book.status !== "exportable" && book.status !== "listed");
  const exportable = books.filter((book) => book.status === "exportable");
  const incompleteChildren = childItems.filter((child) => child.completeness < 85);
  const workspaceSubmissions = submissionItems.filter((item) => item.workspaceId === workspace.id);
  const pendingSubmissions = workspaceSubmissions.filter((item) => item.status === "submitted" || !item.privacyConfirmed);

  if (workspace.role === "school_admin") {
    return [
      ...pendingSubmissions.slice(0, 2).map((item) => ({
        title: `确认投稿《${item.title}》`,
        copy: item.privacyConfirmed ? "投稿已提交，等待审核结果同步。" : "隐私确认未完成，需先核对儿童信息和园所授权。",
        to: "../admin/submissions",
        badge: item.privacyConfirmed ? "审核中" : "隐私待确认",
        tone: item.privacyConfirmed ? "warn" as const : "danger" as const,
      })),
      {
        title: "邀请老师加入园所",
        copy: "补齐协作成员后，老师可以在授权班级内创建绘本和维护儿童资料。",
        to: "../admin/members",
        badge: "协作",
        tone: "info",
      },
      ...incompleteChildren.slice(0, 1).map((child) => ({
        title: `补齐 ${child.nickname} 的儿童资料`,
        copy: `当前完整度 ${child.completeness}%，会影响定制绘本的个性化质量。`,
        to: `../children/${child.id}`,
        badge: "资料不足",
        tone: "warn" as const,
      })),
    ];
  }

  if (workspace.role === "school_teacher") {
    return [
      ...incompleteChildren.slice(0, 1).map((child) => ({
        title: `检查 ${child.nickname} 的定制资料`,
        copy: `${child.classroom || "授权班级"} · ${child.focus}，资料完整后可稳定生成定制副本。`,
        to: `../children/${child.id}`,
        badge: "档案检查",
        tone: "warn" as const,
      })),
      ...exportable.slice(0, 1).map((book) => ({
        title: `基于《${book.title}》生成定制绘本`,
        copy: "先选择孩子，再检查档案、定制强度和生成方案。",
        to: `../storybooks/${book.id}/customize`,
        badge: "可定制",
        tone: "good" as const,
      })),
      {
        title: "创建新的普通绘本",
        copy: "适合班级共读、规则引导和主题活动，之后可以继续派生定制绘本。",
        to: "../storybooks/new",
        badge: "创作",
        tone: "info",
      },
    ];
  }

  return [
    ...editable.slice(0, 1).map((book) => ({
      title: `继续编辑《${book.title}》`,
      copy: storybookNextAction(book),
      to: `../storybooks/${book.id}`,
      badge: storybookStatusLabel[book.status],
      tone: statusTone(book.status),
    })),
    ...exportable.slice(0, 1).map((book) => ({
      title: `导出或定制《${book.title}》`,
      copy: "这本普通绘本已经可导出，也可以继续为单个孩子生成定制副本。",
      to: `../storybooks/${book.id}`,
      badge: "可继续使用",
      tone: "good" as const,
    })),
    {
      title: childItems.length > 0 ? "维护孩子资料" : "创建第一个孩子资料",
      copy: childItems.length > 0 ? "保持兴趣、特质和关注点更新，定制绘本会更贴近孩子。" : "有孩子资料后，普通绘本才能更自然地派生成定制版本。",
      to: "../children",
      badge: "定制准备",
      tone: childItems.length > 0 ? "info" : "warn",
    },
  ];
}

function getMetrics(workspace: Workspace, books: Storybook[], childItems: ChildProfile[], submissionItems: MarketplaceSubmission[]): DashboardMetric[] {
  const editableCount = books.filter((book) => book.status !== "exportable" && book.status !== "listed").length;
  const exportableCount = books.filter((book) => book.status === "exportable").length;
  const readyChildren = childItems.filter((child) => child.completeness >= 80).length;
  const pendingSubmissionCount = submissionItems.filter((item) => item.workspaceId === workspace.id && (item.status === "submitted" || !item.privacyConfirmed)).length;

  if (workspace.role === "school_admin") {
    return [
      { label: "待确认投稿", value: pendingSubmissionCount, copy: "需要审核进度或隐私确认", tone: pendingSubmissionCount > 0 ? "warn" : "good" },
      { label: "园所绘本", value: books.length, copy: "当前园所空间作品总数", tone: "info" },
      { label: "可用于定制儿童", value: readyChildren, copy: "资料完整度达到 80% 以上", tone: "good" },
      { label: "可导出绘本", value: exportableCount, copy: "可分享或派生定制副本", tone: "good" },
    ];
  }

  if (workspace.role === "school_teacher") {
    return [
      { label: "授权班级儿童", value: childItems.length, copy: "可维护资料并生成定制绘本", tone: "good" },
      { label: "可定制母本", value: exportableCount, copy: "已完成且可继续派生", tone: "good" },
      { label: "继续编辑", value: editableCount, copy: "需要确认文字、角色或插图", tone: editableCount > 0 ? "warn" : "good" },
      { label: "本空间绘本", value: books.length, copy: "班级共读和规则引导内容", tone: "info" },
    ];
  }

  return [
    { label: "可继续编辑", value: editableCount, copy: "草稿、编辑中或待确认绘本", tone: editableCount > 0 ? "warn" : "good" },
    { label: "可导出绘本", value: exportableCount, copy: "已完成，可分享或派生", tone: "good" },
    { label: "孩子资料", value: childItems.length, copy: "用于生成定制绘本", tone: childItems.length > 0 ? "good" : "warn" },
    { label: "普通绘本", value: books.filter((book) => book.type === "plain").length, copy: "可作为定制绘本母本", tone: "info" },
  ];
}

function getQuickActions(workspace: Workspace, childItems: ChildProfile[]) {
  if (workspace.role === "school_admin") {
    return [
      { icon: <UserPlus />, title: "邀请老师", copy: "管理成员和授权班级", to: "../admin/members" },
      { icon: <ClipboardCheck />, title: "处理投稿", copy: "审核预览和隐私确认", to: "../admin/submissions" },
      { icon: <UsersRound />, title: "查看班级", copy: "检查班级与儿童资料", to: "../admin/classes" },
    ];
  }

  if (workspace.role === "school_teacher") {
    return [
      { icon: <BookOpen />, title: "查看园所绘本", copy: "全部普通与定制绘本", to: "../storybooks" },
      { icon: <UsersRound />, title: "检查儿童资料", copy: `${childItems.length} 份授权资料`, to: "../children" },
      { icon: <Library />, title: "从市场复制", copy: "复用平台模板或园所作品", to: "../marketplace" },
    ];
  }

  return [
    { icon: <BookOpen />, title: "新建普通绘本", copy: "先确认故事方案和角色", to: "../storybooks/new" },
    { icon: <Library />, title: "从市场复制", copy: "复用平台模板或园所作品", to: "../marketplace" },
    { icon: <UsersRound />, title: "维护孩子资料", copy: "为定制绘本做准备", to: "../children" },
  ];
}

function getCreationActions(books: Storybook[]) {
  const firstExportable = books.find((book) => book.status === "exportable");

  return [
    {
      icon: <BookOpen />,
      title: "创建普通绘本",
      copy: "从故事需求开始，确认方案、角色和分页，生成可导出的班级共读绘本。",
      to: "../storybooks/new",
      label: "开始创建",
      tone: "info" as const,
    },
    {
      icon: <FileCheck2 />,
      title: "生成定制绘本",
      copy: firstExportable
        ? `基于《${firstExportable.title}》选择孩子，检查档案后生成独立副本。`
        : "先完成一本可导出的普通绘本，再为单个孩子生成定制副本。",
      to: firstExportable ? `../storybooks/${firstExportable.id}/customize` : "../storybooks",
      label: firstExportable ? "选择孩子定制" : "先选择母本",
      tone: firstExportable ? "good" as const : "warn" as const,
    },
  ];
}

function dashboardCopy(workspace: Workspace) {
  if (workspace.role === "school_admin") return "今天先看园所协作、投稿隐私和班级资料，再安排绘本生产。";
  if (workspace.role === "school_teacher") return "聚焦授权班级：准备普通绘本，检查儿童资料，再生成定制副本。";
  return "从普通绘本开始，继续编辑、导出或基于孩子资料生成定制绘本。";
}

function formatBytes(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let amount = value;
  let unitIndex = 0;
  while (amount >= 1024 && unitIndex < units.length - 1) {
    amount /= 1024;
    unitIndex += 1;
  }
  const digits = amount >= 10 || unitIndex === 0 ? 0 : 1;
  return `${amount.toFixed(digits)} ${units[unitIndex]}`;
}

function quotaTone(quota: WorkspaceStorageQuota): "neutral" | "good" | "warn" | "danger" | "info" {
  if (quota.exceeded) return "danger";
  if (quota.warning) return "warn";
  return "good";
}

function quotaCopy(quota: WorkspaceStorageQuota) {
  if (quota.exceeded) return "当前空间已超过存储限额，新的图片生成和 PDF 导出会被阻止。";
  if (quota.warning) return "当前空间接近存储上限，建议清理不再使用的导出和插图。";
  return "当前空间容量充足，可以继续生成插图和导出 PDF。";
}

export function DashboardPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const [remoteData, setRemoteData] = useState<DashboardData | null>(null);
  const [generationJobs, setGenerationJobs] = useState<GenerationJob[]>([]);
  const [storageQuota, setStorageQuota] = useState<WorkspaceStorageQuota>(() => defaultStorageQuota(workspace));
  const [userStorageQuota, setUserStorageQuota] = useState<UserStorageQuota>(() => defaultUserStorageQuota());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const books = remoteData?.storybooks ?? [];
  const childItems = remoteData?.children ?? [];
  const submissionItems = remoteData?.submissions ?? [];
  const primary = getPrimary(workspace, books, childItems);
  const tasks = getTasks(workspace, books, childItems, submissionItems).slice(0, 4);
  const metrics = getMetrics(workspace, books, childItems, submissionItems);
  const quickActions = getQuickActions(workspace, childItems);
  const creationActions = getCreationActions(books);
  const showCreationLaunch = workspace.type === "school";
  const isPersonal = workspace.type === "personal";
  const isEmptyAccount =
    books.length === 0 &&
    generationJobs.length === 0 &&
    childItems.length === 0 &&
    submissionItems.length === 0;

  useEffect(() => {
    let mounted = true;
    setLoading(true);
    setRemoteData(null);
    setGenerationJobs([]);
    setStorageQuota(defaultStorageQuota(workspace));
    setUserStorageQuota(defaultUserStorageQuota());
    setError("");
    // 分区降级：主数据失败才报错；任务、配额等辅助数据失败时保留默认值，不拖垮整个工作台。
    Promise.allSettled([
      dashboard(workspace.id),
      listGenerationJobsPage(workspace.id, { limit: 12, offset: 0 }),
      getWorkspaceStorageQuota(workspace.id),
      getUserStorageQuota(),
    ])
      .then(([dataResult, jobsResult, quotaResult, userQuotaResult]) => {
        if (!mounted) return;
        if (dataResult.status === "fulfilled") {
          setRemoteData(dataResult.value);
          setError("");
        } else {
          setRemoteData(null);
          setError(dataResult.reason instanceof Error ? dataResult.reason.message : "无法读取工作台数据");
        }
        if (jobsResult.status === "fulfilled") setGenerationJobs(jobsResult.value.data);
        if (quotaResult.status === "fulfilled") setStorageQuota(quotaResult.value);
        if (userQuotaResult.status === "fulfilled") setUserStorageQuota(userQuotaResult.value);
      })
      .finally(() => {
        if (mounted) setLoading(false);
      });
    return () => {
      mounted = false;
    };
  }, [workspace.id, workspace.type]);

  if (loading) {
    return <div className="page-stack"><EmptyState title="正在读取工作台" copy="正在从后端加载当前空间的数据。" /></div>;
  }

  if (error) {
    return <div className="page-stack"><EmptyState title="工作台加载失败" copy={error} /></div>;
  }

  return (
    <div className="page-stack dashboard-page">
      <PageHeader
        eyebrow={workspace.type === "personal" ? "个人工作台" : "园所工作台"}
        title={workspace.type === "personal" ? "我的工作台" : `${workspace.name} 工作台`}
        copy={dashboardCopy(workspace)}
      />

      {showCreationLaunch && (
        <section className="creation-launch">
          <div className="creation-launch-head">
            <p className="eyebrow">Create</p>
            <h2>绘本生产入口</h2>
            <p>普通绘本用于班级共读和主题活动；定制绘本基于普通绘本和儿童资料生成。</p>
          </div>
          <div className="creation-launch-grid">
            {creationActions.map((action) => (
              <Link className="creation-card" to={action.to} key={action.title}>
                <div className="creation-icon">{action.icon}</div>
                <div>
                  <Badge tone={action.tone}>{action.title}</Badge>
                  <h3>{action.title}</h3>
                  <p>{action.copy}</p>
                </div>
                <span className="creation-cta">
                  {action.label}
                  <ArrowRight size={16} />
                </span>
              </Link>
            ))}
          </div>
        </section>
      )}

      <section className="today-focus">
        <div className="focus-icon">{primary.icon}</div>
        <div className="focus-copy">
          <Badge tone={workspace.role === "school_admin" ? "warn" : "info"}>{primary.meta}</Badge>
          <h2>{primary.title}</h2>
          <p>{primary.copy}</p>
        </div>
        <Link className="button primary focus-action" to={primary.action.to}>
          {primary.action.label}
          <ArrowRight size={16} />
        </Link>
      </section>

      {!isEmptyAccount && (
        <section className="metric-grid">
          {metrics.map((metric) => (
            <Card key={metric.label}>
              <Badge tone={metric.tone}>{metric.label}</Badge>
              <strong>{metric.value}</strong>
              <p>{metric.copy}</p>
            </Card>
          ))}
        </section>
      )}

      <section className="dashboard-main-grid">
        <Card className="task-panel">
          <div className="section-head">
            <div><p className="eyebrow">Today</p><h2>待办队列</h2></div>
            <Badge tone={tasks.length > 0 ? "warn" : "good"}>{tasks.length > 0 ? `${tasks.length} 项` : "已清空"}</Badge>
          </div>
          {tasks.length === 0 ? (
            <EmptyState title="今天没有待办" copy="当前空间没有需要立即处理的绘本、投稿或资料问题。" />
          ) : (
            <div className="task-list">
              {tasks.map((task) => (
                <Link key={`${task.title}-${task.to}`} className="task-row" to={task.to}>
                  <Clock3 size={18} />
                  <div>
                    <strong>{task.title}</strong>
                    <span>{task.copy}</span>
                  </div>
                  <Badge tone={task.tone}>{task.badge}</Badge>
                </Link>
              ))}
            </div>
          )}
        </Card>

        <Card className="quick-panel">
          <div className="section-head">
            <div><p className="eyebrow">Actions</p><h2>常用操作</h2></div>
          </div>
          <div className="action-grid compact-actions">
            {quickActions.map((action) => (
              <Link className="action-card" to={action.to} key={action.title}>
                {action.icon}
                {action.title}
                <span>{action.copy}</span>
              </Link>
            ))}
          </div>
        </Card>
      </section>

      {isPersonal && isEmptyAccount ? (
        <Card>
          <div className="section-head">
            <div><p className="eyebrow">Start</p><h2>三步开始</h2></div>
          </div>
          <div className="action-grid compact-actions">
            <Link className="action-card" to="../children">
              <UsersRound />
              创建孩子资料
              <span>记录兴趣、特质和关注点，用于生成定制绘本</span>
            </Link>
            <Link className="action-card" to="../storybooks/new">
              <BookOpen />
              新建普通绘本
              <span>确认故事方案、角色和分页后生成</span>
            </Link>
            <Link className="action-card" to="../marketplace">
              <Library />
              从市场复制
              <span>复用平台模板，快速获得第一本绘本</span>
            </Link>
          </div>
        </Card>
      ) : (
        <>
          <Card>
            <div className="section-head">
              <div><p className="eyebrow">Generation</p><h2>最近生成任务</h2></div>
              <Badge tone={generationJobs.some((job) => job.status === "failed") ? "danger" : generationJobs.length ? "good" : "neutral"}>
                {generationJobs.length ? `${generationJobs.length} 条` : "暂无任务"}
              </Badge>
            </div>
            {generationJobs.length === 0 ? (
              <EmptyState title="还没有生成任务" copy="创建普通绘本、生成插图或生成定制方案后，这里会显示最近任务状态。" />
            ) : (
              <div className="compact-list generation-job-list">
                {generationJobs.slice(0, 5).map((job) => {
                  const row = (
                    <>
                      <div>
                        <strong>{generationJobTitle(job)}</strong>
                        <span>{generationJobCopy(job)}</span>
                        <small>{generationJobNextAction(job)}</small>
                        <small>任务 {job.id.slice(0, 8)} · {generationJobTime(job)}</small>
                      </div>
                      <Badge tone={statusTone(job.status)}>{generationJobStatusLabel[job.status] || job.status}</Badge>
                    </>
                  );
                  return job.storybookId ? (
                    <Link key={job.id} to={`../storybooks/${job.storybookId}`} className="compact-row dashboard-recent-row">
                      {row}
                    </Link>
                  ) : (
                    <div key={job.id} className="compact-row dashboard-recent-row static">
                      {row}
                    </div>
                  );
                })}
              </div>
            )}
          </Card>

          <Card>
            <div className="section-head">
              <div><p className="eyebrow">Recent</p><h2>最近更新</h2></div>
              <Link className="button secondary" to="../storybooks">查看全部</Link>
            </div>
            {books.length === 0 ? (
              <EmptyState title="还没有绘本" copy="从空白创建普通绘本，或先去市场复制一个模板。" />
            ) : (
              <div className="compact-list">
                {books.slice(0, 4).map((book) => (
                  <Link key={book.id} to={`../storybooks/${book.id}`} className="compact-row dashboard-recent-row">
                    <div>
                      <strong>{book.title}</strong>
                      <span>{book.useScene} · {book.ageGroup} · {book.updatedAt}</span>
                      <small>{storybookNextAction(book)}</small>
                    </div>
                    <Badge tone={statusTone(book.status)}>{storybookStatusLabel[book.status]}</Badge>
                  </Link>
                ))}
              </div>
            )}
          </Card>
        </>
      )}

      {storageQuota.warning || storageQuota.exceeded ? (
        <Card className="storage-quota-card">
          <div>
            <p className="eyebrow">Storage</p>
            <h2>当前空间容量</h2>
            <p>{quotaCopy(storageQuota)}</p>
          </div>
          <div className="storage-quota-meter">
            <div className="storage-quota-meta">
              <strong>{formatBytes(storageQuota.usedBytes)} / {formatBytes(storageQuota.quotaBytes)}</strong>
              <Badge tone={quotaTone(storageQuota)}>{Math.round(storageQuota.usedPercent)}%</Badge>
            </div>
            <div className="quota-track" aria-label="当前空间存储使用比例">
              <span style={{ width: `${Math.min(100, Math.max(0, storageQuota.usedPercent))}%` }} />
            </div>
            <small>剩余 {formatBytes(storageQuota.remainingBytes)} · 预警线 {Math.round(storageQuota.warningPercent)}%</small>
            <small>我的容量 {formatBytes(userStorageQuota.usedBytes)} / {formatBytes(userStorageQuota.quotaBytes)} · 个人空间 {userStorageQuota.personalWorkspaceCount} 个</small>
          </div>
        </Card>
      ) : (
        <p className="storage-quota-inline">
          空间容量 {formatBytes(storageQuota.usedBytes)} / {formatBytes(storageQuota.quotaBytes)}（{Math.round(storageQuota.usedPercent)}%）· 容量充足
        </p>
      )}
    </div>
  );
}
