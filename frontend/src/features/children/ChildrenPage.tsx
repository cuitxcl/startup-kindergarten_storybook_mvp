import { FormEvent, useEffect, useState } from "react";
import { Link, useOutletContext } from "react-router-dom";
import {
  confirmParentIntake,
  createChild,
  createParentIntakeLink,
  listClassroomsPage,
  listChildrenPage,
  listParentIntakeLinksPage,
  listParentIntakesPage,
  revokeActiveParentIntakeLinks,
  revokeParentIntakeLink,
  shouldUseApi,
  type PaginationMeta,
} from "../../api/client";
import { Badge, Card, EmptyState, Modal, Notice, PageHeader } from "../../components/ui";
import { children } from "../../data/mock";
import type { ChildProfile, Classroom, ParentIntake, ParentIntakeLink, Workspace } from "../../types/domain";

const PAGE_SIZE = 12;

function splitTags(value: string) {
  return value
    .split(/[、,，]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

export function ChildrenPage() {
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const [open, setOpen] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [remoteRows, setRemoteRows] = useState<ChildProfile[]>([]);
  const [offset, setOffset] = useState(0);
  const [pageMeta, setPageMeta] = useState<PaginationMeta | null>(null);
  const [intakes, setIntakes] = useState<ParentIntake[]>([]);
  const [intakeOffset, setIntakeOffset] = useState(0);
  const [intakeMeta, setIntakeMeta] = useState<PaginationMeta | null>(null);
  const [intakeLinks, setIntakeLinks] = useState<ParentIntakeLink[]>([]);
  const [classroomOptions, setClassroomOptions] = useState<Classroom[]>([]);
  const [intakeLinkOffset, setIntakeLinkOffset] = useState(0);
  const [intakeLinkMeta, setIntakeLinkMeta] = useState<PaginationMeta | null>(null);
  const [intakeLinkLoading, setIntakeLinkLoading] = useState(false);
  const [intakeLinkStatus, setIntakeLinkStatus] = useState<"" | "active" | "revoked" | "expired">("");
  const [classroomFilter, setClassroomFilter] = useState("");
  const [loading, setLoading] = useState(shouldUseApi);
  const [error, setError] = useState("");
  const [confirmingId, setConfirmingId] = useState("");
  const [creatingLink, setCreatingLink] = useState(false);
  const [revokingLinkId, setRevokingLinkId] = useState("");
  const [revokingActiveLinks, setRevokingActiveLinks] = useState(false);
  const [linkExpiry, setLinkExpiry] = useState<"none" | "7d">("7d");
  const [form, setForm] = useState({
    nickname: "",
    ageGroup: "3-4 岁",
    interests: "",
    traits: "",
    focus: "",
  });
  const rows = shouldUseApi ? remoteRows : children.filter((item) => item.workspaceId === workspace.id);
  const initialLoading = loading && (!shouldUseApi || remoteRows.length === 0);
  const canManageParentIntakes = workspace.type === "school" && workspace.role === "school_admin";
  const pendingIntakeCount = intakes.filter((item) => item.status === "submitted").length;
  const summaryItems = [
    { label: "儿童总数", value: shouldUseApi ? pageMeta?.total ?? rows.length : rows.length, copy: "当前空间可用于定制的儿童资料", tone: "info" as const },
    { label: "待确认提交", value: pendingIntakeCount, copy: "家长提交后等待老师确认", tone: pendingIntakeCount ? "warn" as const : "neutral" as const },
    { label: "高完整度", value: rows.filter((item) => item.completeness >= 80).length, copy: "足够支撑稳定定制", tone: "good" as const },
    { label: "可继续补充", value: rows.filter((item) => item.completeness < 80).length, copy: "需要补齐兴趣或关注点", tone: "neutral" as const },
  ];
  const addLabel = workspace.type === "personal" ? "新增孩子资料" : "新增儿童档案";
  const addButton = <button className="button primary" type="button" onClick={() => setOpen(true)}>{addLabel}</button>;
  const activeIntakeLinkCount = intakeLinks.filter((link) => link.status === "active").length;
  const hasActiveIntakeLinks = activeIntakeLinkCount > 0 || (intakeLinkStatus === "active" && (intakeLinkMeta?.total ?? 0) > 0);

  useEffect(() => {
    setOffset(0);
    setIntakeOffset(0);
    setIntakeLinkOffset(0);
  }, [workspace.id]);

  useEffect(() => {
    setIntakeOffset(0);
    setIntakeLinkOffset(0);
  }, [classroomFilter, intakeLinkStatus]);

  useEffect(() => {
    if (!shouldUseApi) return;
    let mounted = true;
    setLoading(true);
    if (offset === 0) {
        setRemoteRows([]);
        setIntakeLinks([]);
        setPageMeta(null);
        setIntakeLinkMeta(null);
        if (canManageParentIntakes) setClassroomOptions([]);
        if (intakeOffset === 0) {
          setIntakes([]);
          setIntakeMeta(null);
      }
    }
    setError("");
    Promise.all([
      listChildrenPage(workspace.id, { limit: PAGE_SIZE, offset }),
      canManageParentIntakes
        ? listParentIntakesPage(workspace.id, { classroom: classroomFilter || undefined, limit: PAGE_SIZE, offset: intakeOffset })
        : Promise.resolve({ data: intakes, meta: intakeMeta }),
      offset === 0 && canManageParentIntakes && intakeLinkOffset === 0
        ? listParentIntakeLinksPage(workspace.id, {
            status: intakeLinkStatus || undefined,
            classroom: classroomFilter || undefined,
            limit: PAGE_SIZE,
            offset: 0,
          })
        : Promise.resolve(null),
      offset === 0 && canManageParentIntakes
        ? listClassroomsPage(workspace.id, { limit: 100, offset: 0 })
        : Promise.resolve(null),
    ])
      .then(([page, intakePage, intakeLinkPage, classroomPage]) => {
        if (!mounted) return;
        setRemoteRows((current) => (
          offset === 0
            ? page.data
            : [...current, ...page.data.filter((child) => !current.some((item) => item.id === child.id))]
        ));
        setPageMeta(page.meta);
        setIntakes((current) => (
          intakeOffset === 0
            ? intakePage.data
            : [...current, ...intakePage.data.filter((intake) => !current.some((item) => item.id === intake.id))]
        ));
        setIntakeMeta(intakePage.meta);
        if (intakeLinkPage) {
          setIntakeLinks(intakeLinkPage.data);
          setIntakeLinkMeta(intakeLinkPage.meta);
        }
        if (classroomPage) {
          setClassroomOptions(classroomPage.data.filter((item) => item.status === "active"));
        }
        setError("");
      })
      .catch((err) => {
        if (!mounted) return;
        if (offset === 0) {
          setRemoteRows([]);
          setIntakes([]);
          setIntakeLinks([]);
          setPageMeta(null);
          setIntakeMeta(null);
          setIntakeLinkMeta(null);
        }
        setError(err instanceof Error ? err.message : "无法读取儿童档案");
      })
      .finally(() => {
        if (mounted) setLoading(false);
      });
    return () => {
      mounted = false;
    };
  }, [canManageParentIntakes, classroomFilter, intakeLinkStatus, intakeOffset, offset, workspace.id]);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!shouldUseApi) {
      setOpen(false);
      setNotice("这是 mock 反馈：真实接入后会创建资料，并进入老师确认流程。");
      return;
    }
    try {
      const child = await createChild(workspace.id, {
        nickname: form.nickname,
        ageGroup: form.ageGroup,
        interests: splitTags(form.interests),
        traits: splitTags(form.traits),
        focus: form.focus || "待老师确认",
      });
      setRemoteRows((current) => [child, ...current.filter((item) => item.id !== child.id)]);
      setPageMeta((meta) => meta ? { ...meta, total: meta.total + 1 } : meta);
      setOpen(false);
      setNotice(`已创建 ${child.nickname} 的资料。`);
      setForm({ nickname: "", ageGroup: "3-4 岁", interests: "", traits: "", focus: "" });
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "新增失败，请稍后重试。");
    }
  }

  async function confirmIntake(intake: ParentIntake) {
    if (!shouldUseApi) {
      setNotice(`已确认 ${intake.childNickname} 的家长提交资料。`);
      return;
    }
    setConfirmingId(intake.id);
    try {
      const child = await confirmParentIntake(workspace.id, intake.id, {
        focus: "家长提交资料，老师后续补充关注点",
      });
      setRemoteRows((current) => [child, ...current.filter((item) => item.id !== child.id)]);
      setPageMeta((meta) => meta ? { ...meta, total: meta.total + 1 } : meta);
      setIntakes((current) => current.map((item) => (
        item.id === intake.id
          ? { ...item, status: "confirmed", confirmedChildId: child.id, updatedAt: child.updatedAt }
          : item
      )));
      setNotice(`已确认 ${child.nickname} 的资料，并生成儿童档案。`);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "确认失败，请稍后重试。");
    } finally {
      setConfirmingId("");
    }
  }

  async function createIntakeLink() {
    if (!shouldUseApi) {
      setNotice("这是 mock 反馈：真实接入后会生成一条家长资料收集链接。");
      return;
    }
    setCreatingLink(true);
    try {
      const expiresAt = linkExpiry === "7d"
        ? new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString()
        : undefined;
      const link = await createParentIntakeLink(workspace.id, {
        label: classroomFilter ? `${workspace.name} ${classroomFilter} 家长资料收集` : `${workspace.name} 家长资料收集`,
        classroom: classroomFilter || undefined,
        expiresAt,
      });
      const linkMatchesCurrentFilter =
        (!intakeLinkStatus || link.status === intakeLinkStatus)
        && (!classroomFilter || link.classroom === classroomFilter);
      if (linkMatchesCurrentFilter) {
        setIntakeLinks((current) => [link, ...current.filter((item) => item.id !== link.id)]);
        setIntakeLinkMeta((meta) => meta ? { ...meta, total: meta.total + 1, offset: 0 } : meta);
      }
      const page = await listParentIntakeLinksPage(workspace.id, {
        status: intakeLinkStatus || undefined,
        classroom: classroomFilter || undefined,
        limit: PAGE_SIZE,
        offset: 0,
      });
      setIntakeLinks(page.data.length || !linkMatchesCurrentFilter ? page.data : [link, ...page.data.filter((item) => item.id !== link.id)]);
      setIntakeLinkMeta(page.data.length || !linkMatchesCurrentFilter ? page.meta : { ...page.meta, total: Math.max(page.meta.total, 1) });
      setIntakeLinkOffset(0);
      setNotice(`家长资料链接已生成：${window.location.origin}${link.url}${link.expiresAt ? `，有效期至 ${link.expiresAt}` : ""}`);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "生成链接失败，请稍后重试。");
    } finally {
      setCreatingLink(false);
    }
  }

  async function loadMoreIntakeLinks() {
    if (!shouldUseApi || !intakeLinkMeta?.has_more) return;
    setIntakeLinkLoading(true);
    setNotice(null);
    try {
      const nextOffset = intakeLinkMeta.offset + intakeLinkMeta.limit;
      const page = await listParentIntakeLinksPage(workspace.id, {
        status: intakeLinkStatus || undefined,
        classroom: classroomFilter || undefined,
        limit: PAGE_SIZE,
        offset: nextOffset,
      });
      setIntakeLinks((current) => [
        ...current,
        ...page.data.filter((link) => !current.some((item) => item.id === link.id)),
      ]);
      setIntakeLinkMeta(page.meta);
      setIntakeLinkOffset(nextOffset);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "资料链接加载失败，请稍后重试。");
    } finally {
      setIntakeLinkLoading(false);
    }
  }

  async function revokeIntakeLink(link: ParentIntakeLink) {
    if (!shouldUseApi) {
      setNotice("这是 mock 反馈：真实接入后会撤回这条资料收集链接。");
      return;
    }
    setRevokingLinkId(link.id);
    try {
      const updated = await revokeParentIntakeLink(workspace.id, link.id);
      setIntakeLinks((current) => (
        intakeLinkStatus === "active"
          ? current.filter((item) => item.id !== updated.id)
          : current.map((item) => (item.id === updated.id ? updated : item))
      ));
      if (intakeLinkStatus === "active") {
        setIntakeLinkMeta((meta) => meta ? { ...meta, total: Math.max(0, meta.total - 1) } : meta);
      }
      setNotice(`家长资料链接已撤回：${updated.label}`);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "撤回链接失败，请稍后重试。");
    } finally {
      setRevokingLinkId("");
    }
  }

  async function revokeAllActiveIntakeLinks() {
    if (!shouldUseApi) {
      setNotice("这是 mock 反馈：真实接入后会停用当前所有可填写家长资料链接。");
      return;
    }
    setRevokingActiveLinks(true);
    try {
      const result = await revokeActiveParentIntakeLinks(workspace.id, { classroom: classroomFilter || undefined });
      const page = await listParentIntakeLinksPage(workspace.id, {
        status: intakeLinkStatus || undefined,
        classroom: classroomFilter || undefined,
        limit: PAGE_SIZE,
        offset: 0,
      });
      setIntakeLinks(page.data);
      setIntakeLinkMeta(page.meta);
      setIntakeLinkOffset(0);
      setNotice(classroomFilter ? `${classroomFilter}：${result.message}` : result.message);
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "批量停用失败，请稍后重试。");
    } finally {
      setRevokingActiveLinks(false);
    }
  }

  async function copyIntakeLink(link: ParentIntakeLink) {
    const fullUrl = absoluteLinkUrl(link.url);
    setNotice(`家长资料链接已准备复制：${fullUrl}`);
    copyText(fullUrl).catch(() => undefined);
  }

  return (
    <div className="page-stack">
      <PageHeader
        eyebrow={workspace.type === "personal" ? "孩子资料" : "儿童档案"}
        title={workspace.type === "personal" ? "我的孩子" : "儿童档案"}
        copy="定制绘本需要称呼、年龄段和至少一个个性化元素。"
        actions={addButton}
      />
      {notice && <Notice title="资料已提交" copy={notice} tone="good" />}
      <section className="list-hero">
        <div>
          <Badge tone="good">定制准备</Badge>
          <h2>资料越完整，定制绘本越稳定</h2>
          <p>优先补齐兴趣、特质和关注点，生成时会用于角色、道具和情节改写。</p>
        </div>
        {addButton}
      </section>
      <section className="metric-grid">
        {summaryItems.map((item) => (
          <Card key={item.label}>
            <Badge tone={item.tone}>{item.label}</Badge>
            <strong>{item.value}</strong>
            <p>{item.copy}</p>
          </Card>
        ))}
      </section>
      {canManageParentIntakes && (
        <Card>
          <div className="section-heading">
            <div>
              <Badge tone="info">家长资料链接</Badge>
              <h2>收集家长补充资料</h2>
              <p>
                生成链接后发给家长填写，提交内容会进入当前园所的待确认资料队列。
                {shouldUseApi && intakeLinkMeta ? ` 已显示 ${intakeLinks.length} / 共 ${intakeLinkMeta.total} 条链接。` : ""}
              </p>
            </div>
            <div className="inline-actions">
              <label>
                班级范围
                <select value={classroomFilter} onChange={(event) => setClassroomFilter(event.target.value)}>
                  <option value="">全部班级</option>
                  {classroomOptions.map((classroom) => (
                    <option key={classroom.id} value={classroom.name}>{classroom.name}</option>
                  ))}
                </select>
              </label>
              <label>
                链接状态
                <select value={intakeLinkStatus} onChange={(event) => setIntakeLinkStatus(event.target.value as "" | "active" | "revoked" | "expired")}>
                  <option value="">全部</option>
                  <option value="active">可填写</option>
                  <option value="revoked">已撤回</option>
                  <option value="expired">已过期</option>
                </select>
              </label>
              <label>
                有效期
                <select value={linkExpiry} onChange={(event) => setLinkExpiry(event.target.value as "none" | "7d")}>
                  <option value="7d">7 天</option>
                  <option value="none">不过期</option>
                </select>
              </label>
              <button className="button primary" type="button" disabled={creatingLink} onClick={createIntakeLink}>
                {creatingLink ? "生成中..." : "生成资料链接"}
              </button>
              <button
                className="button secondary"
                type="button"
                disabled={revokingActiveLinks || !hasActiveIntakeLinks}
                onClick={revokeAllActiveIntakeLinks}
              >
                {revokingActiveLinks ? "停用中..." : classroomFilter ? `停用${classroomFilter}可填写链接` : "停用全部可填写链接"}
              </button>
            </div>
          </div>
          {intakeLinks.length === 0 ? (
            <EmptyState title="还没有资料链接" copy="生成一条链接后，就可以让家长提交孩子称呼、年龄段和兴趣信息。" />
          ) : (
            <div className="table-list">
              {intakeLinks.map((link) => (
                <div className="table-row" key={link.id}>
                  <div>
                    <strong>{link.label}</strong>
                    <span>{link.classroom || "全部班级"} · {window.location.origin}{link.url}{link.expiresAt ? ` · 有效期至 ${link.expiresAt}` : " · 不过期"}</span>
                    <span>打开 {link.accessCount} 次{link.lastAccessedAt ? ` · 最近打开 ${link.lastAccessedAt}` : " · 尚未打开"}</span>
                  </div>
                  <Badge tone={link.status === "active" ? "good" : "neutral"}>{link.status === "active" ? "可填写" : link.status === "revoked" ? "已撤回" : link.status === "expired" ? "已过期" : link.status}</Badge>
                  {link.status === "active" ? (
                    <div className="inline-actions">
                      <Link className="button secondary" to={link.url}>打开链接</Link>
                      <button className="button secondary" type="button" onClick={() => copyIntakeLink(link)}>复制链接</button>
                      <button className="button secondary" type="button" disabled={revokingLinkId === link.id} onClick={() => revokeIntakeLink(link)}>
                        {revokingLinkId === link.id ? "撤回中..." : "撤回链接"}
                      </button>
                    </div>
                  ) : (
                    <span>已停止收集</span>
                  )}
                </div>
              ))}
            </div>
          )}
          {shouldUseApi && intakeLinkMeta?.has_more && (
            <div className="inline-actions">
              <button className="button secondary" type="button" disabled={intakeLinkLoading} onClick={loadMoreIntakeLinks}>
                {intakeLinkLoading ? "加载中..." : "继续加载链接"}
              </button>
            </div>
          )}
        </Card>
      )}
      {canManageParentIntakes && intakes.length > 0 && (
        <Card>
          <div className="section-heading">
            <div>
              <Badge tone="warn">家长提交</Badge>
              <h2>待老师确认的儿童资料</h2>
              <p>
                确认后会写入当前园所空间的儿童档案，之后可用于生成定制绘本。
                {classroomFilter ? ` 当前只看 ${classroomFilter}。` : ""}
                {shouldUseApi && intakeMeta ? ` 已显示 ${intakes.length} / 共 ${intakeMeta.total} 条。` : ""}
              </p>
            </div>
            {shouldUseApi && intakeMeta?.has_more && (
              <button className="button secondary" type="button" disabled={loading} onClick={() => setIntakeOffset((value) => value + PAGE_SIZE)}>
                {loading ? "加载中..." : "继续加载提交"}
              </button>
            )}
          </div>
          <div className="table-list">
            {intakes.map((intake) => (
              <div className="table-row" key={intake.id}>
                <div><strong>{intake.childNickname}</strong><span>{intake.classroom || "未分班"} · {intake.ageGroup} · {intake.createdAt}</span></div>
                <span>{intake.interests.length ? intake.interests.join("、") : "未填写兴趣"}</span>
                <Badge tone={intake.status === "submitted" ? "warn" : "good"}>
                  {intake.status === "submitted" ? "待确认" : "已确认"}
                </Badge>
                {intake.status === "submitted" ? (
                  <button
                    className="button secondary"
                    type="button"
                    disabled={confirmingId === intake.id}
                    onClick={() => confirmIntake(intake)}
                  >
                    {confirmingId === intake.id ? "确认中..." : "确认入档"}
                  </button>
                ) : (
                  <Link className="button secondary" to={intake.confirmedChildId || ""}>查看档案</Link>
                )}
              </div>
            ))}
          </div>
        </Card>
      )}
      {initialLoading ? (
        <EmptyState title="正在读取资料" copy="正在从后端加载当前空间的儿童档案。" />
      ) : error && rows.length === 0 ? (
        <EmptyState title="儿童档案加载失败" copy={error} />
      ) : rows.length === 0 ? (
        <EmptyState title="还没有资料" copy="先新增孩子资料，之后就能基于普通绘本生成定制版本。" action={addButton} />
      ) : (
        <>
          {error && <Notice title="儿童档案更新失败" copy={error} tone="danger" />}
          {shouldUseApi && pageMeta && (
            <Card>
              <div className="section-head">
                <div>
                  <p className="eyebrow">档案列表</p>
                  <h2>已显示 {rows.length} / 共 {pageMeta.total} 份资料</h2>
                </div>
                {pageMeta.has_more && (
                  <button className="button secondary" type="button" disabled={loading} onClick={() => setOffset((value) => value + PAGE_SIZE)}>
                    {loading ? "加载中..." : "继续加载"}
                  </button>
                )}
              </div>
            </Card>
          )}
          <Card>
            <div className="table-list">
              {rows.map((child) => (
                <Link className="table-row" to={child.id} key={child.id}>
                  <div><strong>{child.nickname}</strong><span>{child.classroom || "个人空间"} · {child.ageGroup}</span></div>
                  <span>{child.interests.join("、")}</span>
                  <span>{child.focus}</span>
                  <Badge tone={child.completeness > 80 ? "good" : "warn"}>完整度 {child.completeness}%</Badge>
                </Link>
              ))}
            </div>
          </Card>
        </>
      )}
      {open && (
        <Modal title={addLabel} onClose={() => setOpen(false)}>
          <form onSubmit={submit}>
            <label>孩子称呼<input value={form.nickname} onChange={(event) => setForm((current) => ({ ...current, nickname: event.target.value }))} placeholder="例如：小雨" /></label>
            <label>年龄段<select value={form.ageGroup} onChange={(event) => setForm((current) => ({ ...current, ageGroup: event.target.value }))}><option>3-4 岁</option><option>4-5 岁</option><option>5-6 岁</option></select></label>
            <label>兴趣或喜欢的活动<textarea rows={3} value={form.interests} onChange={(event) => setForm((current) => ({ ...current, interests: event.target.value }))} placeholder="例如：贴纸、小兔、唱歌" /></label>
            <label>性格特点<textarea rows={2} value={form.traits} onChange={(event) => setForm((current) => ({ ...current, traits: event.target.value }))} placeholder="例如：慢热、喜欢被鼓励" /></label>
            <label>关注点<textarea rows={2} value={form.focus} onChange={(event) => setForm((current) => ({ ...current, focus: event.target.value }))} placeholder="例如：入园适应和午睡" /></label>
            <div className="modal-actions">
              <button className="button secondary" type="button" onClick={() => setOpen(false)}>取消</button>
              <button className="button primary" type="submit">确认新增</button>
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
    throw new Error("浏览器没有允许复制，请打开链接后手动复制地址。");
  }
}
