import { FormEvent, useEffect, useState } from "react";
import { useLocation, useParams } from "react-router-dom";
import { createShareExport, downloadShareExportFile, getPublicParentIntakeLink, getShareExport, getSharedStorybook, submitParentIntake } from "../../api/client";
import { pollUntilSettled } from "../../utils/generation";
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
  const queryWorkspaceName = params.get("workspaceName") || "当前园所";
  const [link, setLink] = useState<PublicParentIntakeLink | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState<{ title: string; copy: string; tone: "good" | "danger" } | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [form, setForm] = useState({
    childNickname: "",
    ageGroup: "4-5 岁",
    interests: "",
  });
  const workspaceName = link?.workspaceName || queryWorkspaceName;

  useEffect(() => {
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

  if (error || !link) {
    return <main className="link-page"><EmptyState title="家长资料链接不可用" copy={error || "没有找到这条资料收集链接。"} /></main>;
  }

  if (link.status !== "active") {
    return (
      <main className="link-page">
        <EmptyState
          title={link.status === "revoked" ? "家长资料链接已撤回" : "家长资料链接已过期"}
          copy={link.status === "revoked" ? "老师已经撤回这条资料收集链接，请联系老师获取新的链接。" : "这条资料收集链接已超过有效期，请联系老师重新生成。"}
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
        {link.expiresAt && <p className="task-summary">链接有效期至：{link.expiresAt}</p>}
        {(workspaceId || link?.workspaceId) && <p className="task-summary">提交目标空间：{workspaceName}</p>}
        {link.classroom && <p className="task-summary">提交目标班级：{link.classroom}</p>}
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
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState<{ title: string; copy: string; tone: "good" | "danger" } | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportFileUrl, setExportFileUrl] = useState("");
  const [exportBlobUrl, setExportBlobUrl] = useState("");

  useEffect(() => {
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
    if (!book || !token) return;
    setExporting(true);
    setNotice(null);
    setExportFileUrl("");
    if (exportBlobUrl) {
      window.URL.revokeObjectURL(exportBlobUrl);
      setExportBlobUrl("");
    }
    try {
      const job = await createShareExport(token);
      // 统一轮询口径：1s 间隔、90s 预算、切后台自动暂停（替代原 5 次×0.7s 的短轮询）
      const settledJob = await pollUntilSettled(() => getShareExport(token, job.id), job, { timeoutMs: 90_000 });
      if (settledJob.fileUrl) {
        setExportFileUrl(settledJob.fileUrl);
        const file = await downloadShareExportFile(token, settledJob.id);
        const url = window.URL.createObjectURL(file);
        setExportBlobUrl(url);
      }
      setNotice({
        title: settledJob.status === "failed" ? "导出失败" : settledJob.fileUrl ? "PDF 已准备下载" : "PDF 还在生成中",
        copy: settledJob.fileUrl
          ? "点击下方的「打开 PDF」即可查看或保存。"
          : settledJob.status === "failed"
            ? "PDF 生成没有成功，请稍后重新点击下载。"
            : "系统仍在生成 PDF，稍后可重新点击下载。",
        tone: settledJob.status === "failed" ? "danger" : "good",
      });
    } catch (err) {
      setNotice({ title: "导出失败", copy: err instanceof Error ? err.message : "请稍后重试", tone: "danger" });
    } finally {
      setExporting(false);
    }
  };

  if (loading) {
    return <main className="link-page"><EmptyState title="正在加载绘本" copy="正在打开老师分享的绘本链接。" /></main>;
  }

  if (error || !book) {
    return <main className="link-page"><EmptyState title="分享链接不可用" copy={error || "没有找到这本分享绘本。"} /></main>;
  }

  return (
    <main className="link-page">
      <Card className="link-card">
        <Badge tone="good">家庭分享版</Badge>
        <h1>{book.title}</h1>
        <div className="storybook-preview-art"><span>{book.coverTone}</span><strong>{book.title.slice(0, 6)}</strong></div>
        <p>这是一份由老师分享的当前版本绘本。获得链接的人可以查看并下载这本书对应的 PDF，看到的就是老师导出的那一版。</p>
        <div className="review-list">
          <div><span>适用年龄</span><strong>{book.ageGroup}</strong></div>
          <div><span>使用场景</span><strong>{book.useScene}</strong></div>
          <div><span>教学目标</span><strong>{book.teachingGoal}</strong></div>
        </div>
        <section className="shared-story-pages" aria-label="绘本正文">
          <div className="section-head compact">
            <div>
              <p className="eyebrow">绘本正文</p>
              <h2>老师分享的当前版本</h2>
            </div>
            <Badge tone="neutral">{book.pages.length || 0} 页</Badge>
          </div>
          {book.pages.length ? (
            book.pages.map((page) => (
              <article className="shared-story-page" key={page.id}>
                <span>第 {page.pageNumber} 页</span>
                <h3>{page.title}</h3>
                <p>{page.body}</p>
                {page.illustrationPrompt && <small>插图：{page.illustrationPrompt}</small>}
              </article>
            ))
          ) : (
            <p className="task-summary">老师暂未分享分页正文，可先下载 PDF 查看当前版本。</p>
          )}
        </section>
        {notice && <Notice title={notice.title} copy={notice.copy} tone={notice.tone} />}
        {exportBlobUrl && <a className="button secondary" href={exportBlobUrl} target="_blank" rel="noreferrer">打开 PDF</a>}
        {exportFileUrl && !exportBlobUrl && <span className="task-summary">PDF 文件已生成，正在准备安全下载。</span>}
        <button className="button primary" type="button" disabled={exporting} onClick={download}>{exporting ? "准备中..." : "下载 PDF"}</button>
      </Card>
    </main>
  );
}
