import { useEffect, useState } from "react";
import { Link, useOutletContext } from "react-router-dom";
import {
  cancelGenerationJob,
  dashboard,
  getGenerationJob,
  listClassroomsPage,
  listGenerationJobsPage,
  listMembersPage,
  recoverGenerationJobs,
  retryGenerationJob,
  shouldUseApi,
  type DashboardData,
  type GenerationJob,
  type PaginationMeta,
} from "../../api/client";
import { Badge, Card, Notice, PageHeader } from "../../components/ui";
import { children, storybooks, submissions } from "../../data/mock";
import type { Workspace } from "../../types/domain";
import { generationJobNextAction, generationJobStatusLabel, generationJobTypeLabel, generationPrivacyAuditSummary } from "../../utils/labels";

const JOB_PAGE_SIZE = 8;

export function AdminPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const [remoteData, setRemoteData] = useState<DashboardData | null>(null);
  const [memberCount, setMemberCount] = useState(shouldUseApi ? 0 : 3);
  const [classCount, setClassCount] = useState(shouldUseApi ? 0 : 0);
  const [generationJobs, setGenerationJobs] = useState<GenerationJob[]>([]);
  const [jobOffset, setJobOffset] = useState(0);
  const [jobMeta, setJobMeta] = useState<PaginationMeta | null>(null);
  const [generationLoading, setGenerationLoading] = useState(shouldUseApi);
  const [generationError, setGenerationError] = useState("");
  const [overviewError, setOverviewError] = useState("");
  const [recovering, setRecovering] = useState(false);
  const [recoverNotice, setRecoverNotice] = useState<{ title: string; copy: string; tone: "good" | "danger" | "info" } | null>(null);
  const [recoverAgeMinutes, setRecoverAgeMinutes] = useState(15);
  const [recoverLimit, setRecoverLimit] = useState(10);
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);
  const [selectedJob, setSelectedJob] = useState<GenerationJob | null>(null);
  const [jobLoading, setJobLoading] = useState(false);
  const [jobRetrying, setJobRetrying] = useState(false);
  const [jobCanceling, setJobCanceling] = useState(false);
  const [jobNotice, setJobNotice] = useState<{ title: string; copy: string; tone: "good" | "danger" | "info" } | null>(null);

  useEffect(() => {
    if (!shouldUseApi) return;
    let mounted = true;
    setGenerationLoading(true);
    if (jobOffset === 0) {
      setGenerationJobs([]);
      setJobMeta(null);
    }
    Promise.all([
      jobOffset === 0 ? dashboard(workspace.id) : Promise.resolve(remoteData),
      jobOffset === 0 ? listMembersPage(workspace.id, { limit: 1, offset: 0 }) : Promise.resolve(null),
      jobOffset === 0 ? listClassroomsPage(workspace.id, { limit: 1, offset: 0 }) : Promise.resolve(null),
      listGenerationJobsPage(workspace.id, { limit: JOB_PAGE_SIZE, offset: jobOffset }),
    ])
      .then(([data, membersPage, classroomsPage, jobsPage]) => {
        if (!mounted) return;
        setRemoteData(data);
        if (jobOffset === 0) {
          setMemberCount(membersPage?.meta.total ?? 0);
          setClassCount(classroomsPage?.meta.total ?? 0);
        }
        setGenerationJobs((jobs) => (
          jobOffset === 0
            ? jobsPage.data
            : [...jobs, ...jobsPage.data.filter((job) => !jobs.some((item) => item.id === job.id))]
        ));
        setJobMeta(jobsPage.meta);
        setGenerationError("");
        setOverviewError("");
      })
      .catch((err: Error) => {
        if (!mounted) return;
        if (jobOffset === 0) {
          setRemoteData(null);
          setMemberCount(0);
          setClassCount(0);
          setGenerationJobs([]);
          setJobMeta(null);
        }
        setOverviewError(err.message || "园所管理数据加载失败");
        setGenerationError("生成任务加载失败");
      })
      .finally(() => {
        if (mounted) setGenerationLoading(false);
      });
    return () => {
      mounted = false;
    };
  }, [jobOffset, workspace.id]);

  useEffect(() => {
    setJobOffset(0);
  }, [workspace.id]);

  const childCount = shouldUseApi ? remoteData?.children.length ?? 0 : children.filter((item) => item.workspaceId === workspace.id).length;
  const storybookCount = shouldUseApi ? remoteData?.storybooks.length ?? 0 : storybooks.filter((item) => item.workspaceId === workspace.id).length;
  const submissionCount = shouldUseApi ? remoteData?.submissions.length ?? 0 : submissions.filter((item) => item.workspaceId === workspace.id).length;
  const initialGenerationLoading = generationLoading && generationJobs.length === 0;
  const failedJobs = generationJobs.filter((job) => job.status === "failed");
  const runningJobs = generationJobs.filter((job) => job.status === "running");
  const queuedJobs = generationJobs.filter((job) => job.status === "queued");

  async function openJob(jobId: string) {
    setSelectedJobId(jobId);
    setJobLoading(true);
    setJobNotice(null);
    try {
      if (!shouldUseApi) {
        const job = generationJobs.find((item) => item.id === jobId) || null;
        setSelectedJob(job);
        return;
      }
      const job = await getGenerationJob(workspace.id, jobId);
      setSelectedJob(job);
    } catch (err) {
      setJobNotice({ title: "任务读取失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "danger" });
    } finally {
      setJobLoading(false);
    }
  }

  async function recoverJobs() {
    if (!shouldUseApi) {
      setRecoverNotice({ title: "已触发恢复", copy: "这是 mock 反馈：真实接入后会扫描积压生成任务并尝试恢复。", tone: "good" });
      return;
    }
    setRecovering(true);
    setRecoverNotice(null);
    try {
      const result = await recoverGenerationJobs(workspace.id, { ageMinutes: recoverAgeMinutes, limit: recoverLimit });
      setJobOffset(0);
      const refreshed = await listGenerationJobsPage(workspace.id, { limit: JOB_PAGE_SIZE, offset: 0 });
      setGenerationJobs(refreshed.data);
      setJobMeta(refreshed.meta);
      setRecoverNotice({
        title: "生成队列已恢复",
        copy: `${result.message} 已处理 ${result.processed} 个任务。`,
        tone: "good",
      });
    } catch (err) {
      setRecoverNotice({ title: "恢复失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "danger" });
    } finally {
      setRecovering(false);
    }
  }

  async function retrySelectedJob() {
    if (!selectedJob) return;
    setJobRetrying(true);
    setJobNotice(null);
    try {
      const job = await retryGenerationJob(workspace.id, selectedJob.id);
      setSelectedJob(job);
      setGenerationJobs((jobs) => jobs.map((item) => item.id === job.id ? job : item));
      setJobNotice({ title: "已重新生成", copy: `任务 ${job.id.slice(0, 8)} 已再次进入生成流程。`, tone: "good" });
    } catch (err) {
      setJobNotice({ title: "重试失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "danger" });
    } finally {
      setJobRetrying(false);
    }
  }

  async function cancelSelectedJob() {
    if (!selectedJob) return;
    setJobCanceling(true);
    setJobNotice(null);
    try {
      const job = await cancelGenerationJob(workspace.id, selectedJob.id);
      setSelectedJob(job);
      setGenerationJobs((jobs) => jobs.map((item) => item.id === job.id ? job : item));
      setJobNotice({ title: "已取消生成任务", copy: `任务 ${job.id.slice(0, 8)} 不会继续执行。`, tone: "good" });
    } catch (err) {
      setJobNotice({ title: "取消失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "danger" });
    } finally {
      setJobCanceling(false);
    }
  }

  return (
    <div className="page-stack">
      <PageHeader eyebrow="园所管理" title={`${workspace.name} 管理`} copy="按协作、班级资料和市场投稿三类处理园所运营事项。" />
      <section className="admin-overview-grid">
        <Link className="action-card" to="members"><Badge tone="info">成员协作</Badge>邀请老师<span>查看成员、邀请状态和班级授权</span></Link>
        <Link className="action-card" to="classes"><Badge tone="good">班级资料</Badge>班级管理<span>创建班级，查看儿童档案入口</span></Link>
        <Link className="action-card" to="submissions"><Badge tone="warn">市场投稿</Badge>投稿与隐私确认<span>跟踪优秀普通绘本的审核状态</span></Link>
        <Link className="action-card" to="audit-logs"><Badge tone="neutral">审计追踪</Badge>审计日志<span>查看邀请、导出、分享和投稿操作记录</span></Link>
      </section>
      <Card>
        <h2>待处理事项</h2>
        <p className="task-summary">园所管理的重点顺序通常是：先确认隐私，再处理成员授权，最后推进投稿上架和内容复用。</p>
        {overviewError && <Notice title="园所管理数据加载失败" copy={overviewError} tone="danger" />}
      </Card>
      <section className="metric-grid">
        <Card><strong>{memberCount}</strong><p>园所成员</p></Card>
        <Card><strong>{classCount}</strong><p>班级</p></Card>
        <Card><strong>{childCount}</strong><p>儿童档案</p></Card>
        <Card><strong>{storybookCount}</strong><p>园所绘本</p></Card>
        <Card><strong>{submissionCount}</strong><p>市场投稿</p></Card>
      </section>
      <Card>
        <div className="section-head">
          <div>
            <p className="eyebrow">生成队列</p>
            <h2>最近生成任务</h2>
          </div>
        </div>
        <div className="inline-actions">
          <Badge tone={failedJobs.length ? "danger" : runningJobs.length ? "warn" : generationJobs.length ? "good" : "neutral"}>
            {jobMeta ? `${generationJobs.length}/${jobMeta.total} 条` : generationJobs.length ? `${generationJobs.length} 条` : "暂无任务"}
          </Badge>
          <label>
            超时分钟
            <input type="number" min={1} max={120} value={recoverAgeMinutes} onChange={(event) => setRecoverAgeMinutes(Number(event.target.value) || 15)} />
          </label>
          <label>
            最大批次
            <input type="number" min={1} max={50} value={recoverLimit} onChange={(event) => setRecoverLimit(Number(event.target.value) || 10)} />
          </label>
          <button className="button secondary" type="button" disabled={!shouldUseApi || recovering} onClick={recoverJobs}>
            {recovering ? "恢复中..." : "恢复生成队列"}
          </button>
          {shouldUseApi && jobMeta?.has_more && (
            <button className="button secondary" type="button" disabled={generationLoading} onClick={() => setJobOffset((value) => value + JOB_PAGE_SIZE)}>
              {generationLoading ? "加载中..." : "继续加载任务"}
            </button>
          )}
        </div>
        {recoverNotice && <Notice title={recoverNotice.title} copy={recoverNotice.copy} tone={recoverNotice.tone} />}
        {initialGenerationLoading ? (
          <p className="task-summary">正在读取近期生成任务和恢复状态。</p>
        ) : generationError ? (
          <p className="task-summary">{generationError}</p>
        ) : generationJobs.length === 0 ? (
          <p className="task-summary">当前没有需要恢复的生成任务；出现失败或卡住任务后，会在这里看到队列状态。</p>
        ) : (
          <div className="admin-job-grid">
            <div className="compact-list generation-job-list">
              {generationJobs.map((job) => (
                <button className={`compact-row static ${selectedJobId === job.id ? "active" : ""}`} type="button" key={job.id} onClick={() => openJob(job.id)}>
                  <div>
                    <strong>{generationJobTypeLabel[job.jobType] || job.jobType}</strong>
                    <span>{generationJobStatusLabel[job.status] || job.status}</span>
                    <small>{generationJobNextAction(job)}</small>
                    <small>尝试次数 {job.attemptCount} · {generationJobFailureText(job)}</small>
                  </div>
                  <Badge tone={job.status === "failed" ? "danger" : job.status === "running" ? "warn" : "good"}>{job.attemptCount} 次</Badge>
                </button>
              ))}
            </div>
            <Card className="job-detail-card">
              {jobLoading ? (
                <p className="task-summary">正在加载任务详情。</p>
              ) : selectedJob ? (
                <>
                  <div className="section-head">
                    <div>
                      <p className="eyebrow">任务详情</p>
                      <h3>{generationJobTypeLabel[selectedJob.jobType] || selectedJob.jobType}</h3>
                    </div>
                    <Badge tone={selectedJob.status === "failed" ? "danger" : selectedJob.status === "running" ? "warn" : "good"}>{generationJobStatusLabel[selectedJob.status] || selectedJob.status}</Badge>
                  </div>
                  <p className="task-summary">{generationJobNextAction(selectedJob)}</p>
                  <div className="review-list">
                    <div><span>任务编号</span><strong>{selectedJob.id}</strong></div>
                    <div><span>尝试次数</span><strong>{selectedJob.attemptCount}</strong></div>
                    <div><span>最后错误</span><strong>{generationJobFailureText(selectedJob)}</strong></div>
                    <div><span>下次执行</span><strong>{selectedJob.nextRunAt || "无"}</strong></div>
                    <div><span>脱敏审计</span><strong>{generationPrivacyAuditSummary(selectedJob.output) || "未触发脱敏"}</strong></div>
                  </div>
                  <div className="inline-actions">
                    <button className="button secondary" type="button" disabled={jobRetrying || selectedJob.status !== "failed"} onClick={retrySelectedJob}>
                      {jobRetrying ? "重试中..." : "重试失败任务"}
                    </button>
                    <button className="button secondary" type="button" disabled={jobCanceling || !canCancelGenerationJob(selectedJob)} onClick={cancelSelectedJob}>
                      {jobCanceling ? "取消中..." : "取消任务"}
                    </button>
                  </div>
                  {jobNotice && <Notice title={jobNotice.title} copy={jobNotice.copy} tone={jobNotice.tone} />}
                </>
              ) : (
                <p className="task-summary">点击左侧任务查看详情和重试入口。</p>
              )}
            </Card>
          </div>
        )}
      </Card>
      <Card>
        <h2>状态说明</h2>
        <p className="task-summary">成员协作、班级资料和市场投稿是园所管理的三条主线；先处理隐私确认和授权，再推进内容复用。</p>
      </Card>
    </div>
  );
}

function generationJobFailureText(job: GenerationJob) {
  const output = job.output as { error?: { message?: string }; message?: string } | null | undefined;
  return job.lastError || output?.error?.message || output?.message || "无错误信息";
}

function canCancelGenerationJob(job: GenerationJob) {
  return job.status === "queued" || job.status === "failed";
}
