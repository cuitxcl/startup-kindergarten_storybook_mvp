import { FormEvent, useEffect, useState } from "react";
import { useLocation, useParams } from "react-router-dom";
import { createShareExport, downloadShareExportFile, getPublicParentIntakeLink, getShareExport, getSharedStorybook, shouldUseApi, submitParentIntake, type ExportJob } from "../../api/client";
import { Badge, Card, EmptyState, Notice } from "../../components/ui";
import type { PublicParentIntakeLink, Storybook } from "../../types/domain";

function splitTags(value: string) {
  return value
    .split(/[、,\n]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

export function IntakeLinkPage() {
  const { token } = useParams();
  const location = useLocation();
  const params = new URLSearchParams(location.search);
  const workspaceId = params.get("workspaceId") || undefined;
  const queryWorkspaceName = params.get("workspaceName") || (!shouldUseApi && token === "demo-token" ? "星星幼儿园" : "当前园所");
  const [link, setLink] = useState<PublicParentIntakeLink | null>(null);
  const [loading, setLoading] = useState(shouldUseApi);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState<{ title: string; copy: string; tone: "good" | "danger" } | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [form, setForm] = useState({
    childNickname: "",
    ageGroup: "4-5 岁",
    interests: "",
  });
  const workspaceName = shouldUseApi ? link?.workspaceName || "当前园所" : queryWorkspaceName;

  useEffect(() => {
    if (!shouldUseApi) return;
    if (!token) {
      setLink(null);
      setError("缺少家长资料链接 token。");
      setLoading(false);
      return;
    }
    setLoading(true);
    setLink(null);
    setError("");
    setNotice(null);
    getPublicParentIntakeLink(token)
      .then(setLink)
      .catch((err: Error) => {
        setLink(null);
        setError(err.message);
      })
      .finally(() => setLoading(false));
  }, [token]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!shouldUseApi) {
      setNotice({ title: "资料已提交", copy: "老师会先确认资料，再用于生成定制绘本。", tone: "good" });
      return;
    }
    setSubmitting(true);
    setNotice(null);
    try {
      const response = await submitParentIntake({
        linkToken: token,
        workspaceId: link?.workspaceId || workspaceId,
        childNickname: form.childNickname,
        ageGroup: form.ageGroup,
        interests: splitTags(form.interests),
      });
      setNotice({ title: "资料已提交", copy: response.message, tone: "good" });
      setForm({ childNickname: "", ageGroup: "4-5 岁", interests: "" });
    } catch (err) {
      setNotice({ title: "提交失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "danger" });
    } finally {
      setSubmitting(false);
    }
  };

  if (loading) {
    return <main className="link-page"><EmptyState title="正在检查资料链接" copy="正在确认这条家长资料链接是否仍可填写。" /></main>;
  }

  if (error || (shouldUseApi && !link)) {
    return <main className="link-page"><EmptyState title="家长资料链接不可用" copy={error || "没有找到这条资料收集链接。"} /></main>;
  }

  if (shouldUseApi && link?.status !== "active") {
    return (
      <main className="link-page">
        <EmptyState
          title={link?.status === "revoked" ? "家长资料链接已撤回" : "家长资料链接已过期"}
          copy={link?.status === "revoked" ? "老师已经撤回这条资料收集链接，请联系老师获取新的链接。" : "这条资料收集链接已超过有效期，请联系老师重新生成。"}
        />
      </main>
    );
  }

  return (
    <main className="link-page">
      <Card className="link-card">
        <Badge tone="info">{workspaceName}</Badge>
        <h1>填写孩子资料</h1>
        <p>这些资料将提交给老师确认，确认后才会写入儿童档案。</p>
        {shouldUseApi && link?.expiresAt && <p className="task-summary">链接有效期至：{link.expiresAt}</p>}
        {(workspaceId || link?.workspaceId) && <p className="task-summary">提交目标空间：{workspaceName}</p>}
        {shouldUseApi && link?.classroom && <p className="task-summary">提交目标班级：{link.classroom}</p>}
        {notice && <Notice title={notice.title} copy={notice.copy} tone={notice.tone} />}
        <form onSubmit={submit}>
          <label>孩子称呼<input required value={form.childNickname} onChange={(event) => setForm({ ...form, childNickname: event.target.value })} placeholder="例如：乐乐" /></label>
          <label>年龄段<select value={form.ageGroup} onChange={(event) => setForm({ ...form, ageGroup: event.target.value })}><option>3-4 岁</option><option>4-5 岁</option><option>5-6 岁</option></select></label>
          <label>兴趣或喜欢的活动<textarea rows={4} value={form.interests} onChange={(event) => setForm({ ...form, interests: event.target.value })} placeholder="例如：积木车、唱歌、蓝色" /></label>
          <button className="button primary" type="submit" disabled={submitting}>{submitting ? "提交中..." : "提交给老师确认"}</button>
        </form>
      </Card>
    </main>
  );
}

export function ShareLinkPage() {
  const { token } = useParams();
  const [book, setBook] = useState<Storybook | null>(null);
  const [loading, setLoading] = useState(shouldUseApi);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState<{ title: string; copy: string; tone: "good" | "danger" } | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportFileUrl, setExportFileUrl] = useState("");
  const [exportBlobUrl, setExportBlobUrl] = useState("");
  const displayBook = shouldUseApi ? book : {
    id: "mock-share",
    workspaceId: "school-1",
    title: "乐乐学会一起玩",
    type: "custom",
    status: "exportable",
    visibility: "private",
    source: "derived",
    creatorName: "林老师",
    updatedAt: "刚刚",
    ageGroup: "4-5 岁",
    useScene: "家庭共读",
    teachingGoal: "学习轮流和分享",
    coverTone: "温暖、柔和",
    pages: [],
    roles: [],
  } as Storybook;

  useEffect(() => {
    if (!shouldUseApi) return;
    if (!token) {
      setBook(null);
      setError("缺少分享链接 token。");
      setLoading(false);
      return;
    }
    setLoading(true);
    setBook(null);
    setNotice(null);
    setError("");
    getSharedStorybook(token)
      .then(setBook)
      .catch((err: Error) => {
        setBook(null);
        setError(err.message);
      })
      .finally(() => setLoading(false));
  }, [token]);

  useEffect(() => {
    return () => {
      if (exportBlobUrl) window.URL.revokeObjectURL(exportBlobUrl);
    };
  }, [exportBlobUrl]);

  const download = async () => {
    if (!displayBook || !token) return;
    if (!shouldUseApi) {
      setNotice({ title: "PDF 已准备下载", copy: "这是 mock 反馈：真实接入后会下载当前分享版本。", tone: "good" });
      return;
    }
    setExporting(true);
    setNotice(null);
    setExportFileUrl("");
    if (exportBlobUrl) {
      window.URL.revokeObjectURL(exportBlobUrl);
      setExportBlobUrl("");
    }
    try {
      const job = await createShareExport(token);
      const settledJob = await waitForShareExport(job);
      if (settledJob.fileUrl) {
        setExportFileUrl(settledJob.fileUrl);
        const file = await downloadShareExportFile(token, settledJob.id);
        const url = window.URL.createObjectURL(file);
        setExportBlobUrl(url);
        window.open(url, "_blank", "noopener,noreferrer");
      }
      setNotice({
        title: settledJob.status === "failed" ? "导出失败" : settledJob.fileUrl ? "PDF 已准备下载" : "PDF 导出任务已创建",
        copy: settledJob.fileUrl
          ? "PDF 文件已经通过分享链接权限下载，可以打开或保存。"
          : settledJob.status === "failed"
            ? "PDF 生成没有成功，请稍后重新点击下载。"
            : "系统正在生成 PDF，稍后可重新点击下载。",
        tone: settledJob.status === "failed" ? "danger" : "good",
      });
    } catch (err) {
      setNotice({ title: "导出失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "danger" });
    } finally {
      setExporting(false);
    }
  };

  const waitForShareExport = async (initialJob: ExportJob) => {
    if (!token) return initialJob;
    let currentJob = initialJob;
    for (let attempt = 0; attempt < 5 && ["queued", "running"].includes(currentJob.status); attempt += 1) {
      await new Promise((resolve) => window.setTimeout(resolve, 700));
      currentJob = await getShareExport(token, currentJob.id);
    }
    return currentJob;
  };

  if (loading) {
    return <main className="link-page"><EmptyState title="正在加载绘本" copy="正在打开老师分享的绘本链接。" /></main>;
  }

  if (error || !displayBook) {
    return <main className="link-page"><EmptyState title="分享链接不可用" copy={error || "没有找到这本分享绘本。"} /></main>;
  }

  return (
    <main className="link-page">
      <Card className="link-card">
        <Badge tone="good">家庭分享版</Badge>
        <h1>{displayBook.title}</h1>
        <div className="storybook-preview-art"><span>{displayBook.coverTone}</span><strong>{displayBook.title.slice(0, 6)}</strong></div>
        <p>这是一份由老师分享的当前版本绘本。获得链接的人可以查看并下载这本书对应的 PDF，看到的就是老师导出的那一版。</p>
        <div className="review-list">
          <div><span>适用年龄</span><strong>{displayBook.ageGroup}</strong></div>
          <div><span>使用场景</span><strong>{displayBook.useScene}</strong></div>
          <div><span>教学目标</span><strong>{displayBook.teachingGoal}</strong></div>
        </div>
        {notice && <Notice title={notice.title} copy={notice.copy} tone={notice.tone} />}
        {exportBlobUrl && <a className="button secondary" href={exportBlobUrl} target="_blank" rel="noreferrer">打开 PDF</a>}
        {exportFileUrl && !exportBlobUrl && <span className="task-summary">PDF 文件已生成，正在准备安全下载。</span>}
        <button className="button primary" type="button" disabled={exporting} onClick={download}>{exporting ? "准备中..." : "下载 PDF"}</button>
      </Card>
    </main>
  );
}
