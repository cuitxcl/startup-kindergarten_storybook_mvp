import { useEffect, useState } from "react";
import { Link, useNavigate, useOutletContext, useParams } from "react-router-dom";
import { copyMarketplaceTemplate, getMarketplaceTemplate, shouldUseApi } from "../../api/client";
import { Badge, Card, EmptyState, Modal, Notice, PageHeader } from "../../components/ui";
import { marketplaceTemplates } from "../../data/mock";
import type { MarketplaceTemplate, Workspace } from "../../types/domain";

export function MarketplaceDetailPage() {
  const { templateId } = useParams();
  const { workspace } = useOutletContext<{ workspace: Workspace }>();
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const [remoteTemplate, setRemoteTemplate] = useState<MarketplaceTemplate | null>(null);
  const [loading, setLoading] = useState(shouldUseApi);
  const [error, setError] = useState("");
  const [copying, setCopying] = useState(false);
  const [notice, setNotice] = useState<{ title: string; copy: string } | null>(null);
  const template = shouldUseApi ? remoteTemplate : marketplaceTemplates.find((item) => item.id === templateId) || marketplaceTemplates[0];

  useEffect(() => {
    if (!shouldUseApi || !templateId) return;
    let mounted = true;
    setLoading(true);
    setRemoteTemplate(null);
    setError("");
    async function load() {
      try {
        const item = await getMarketplaceTemplate(templateId!);
        if (!mounted) return;
        setRemoteTemplate(item);
        setError("");
      } catch (err) {
        if (!mounted) return;
        setRemoteTemplate(null);
        setError(err instanceof Error ? err.message : "无法读取模板详情");
      } finally {
        if (mounted) setLoading(false);
      }
    }
    load();
    return () => {
      mounted = false;
    };
  }, [templateId]);

  const confirmCopy = async () => {
    if (!template) return;
    if (!shouldUseApi) {
      setOpen(false);
      setNotice({ title: "复制已模拟完成", copy: "真实 API 模式下会创建独立普通绘本副本，并自动打开副本详情。" });
      return;
    }
    setCopying(true);
    setNotice(null);
    try {
      const book = await copyMarketplaceTemplate(workspace.id, template.id);
      navigate(`/app/${workspace.id}/storybooks/${book.id}`);
    } catch (err) {
      setNotice({ title: "复制失败", copy: err instanceof Error ? err.message : "请稍后重试" });
    } finally {
      setCopying(false);
    }
  };

  if (loading) {
    return <EmptyState title="正在加载模板" copy="正在读取绘本模板详情。" />;
  }

  if (error || !template) {
    return <EmptyState title="模板不存在" copy={error || "没有找到这个市场模板。"} action={<Link className="button secondary" to="..">返回市场</Link>} />;
  }

  return (
    <div className="page-stack">
      {notice && <Notice title={notice.title} copy={notice.copy} tone="danger" />}
      <PageHeader
        eyebrow="模板详情"
        title={template.title}
        copy={template.summary}
        actions={<button className="button primary" onClick={() => setOpen(true)}>复制到当前空间</button>}
      />
      <section className="list-hero">
        <div>
          <Badge tone={template.sourceType === "platform" ? "info" : "good"}>{template.sourceLabel}</Badge>
          <h2>复制后会成为当前空间的普通绘本</h2>
          <p>副本可以继续编辑、导出，也可以作为定制绘本母本使用。</p>
        </div>
        <button className="button primary" onClick={() => setOpen(true)}>复制到当前空间</button>
      </section>
      <section className="two-column">
        <Card>
          <div className="storybook-preview-art"><span>{template.sourceLabel}</span><strong>{template.title}</strong></div>
          <div className="tag-list">{template.tags.map((tag) => <Badge key={tag}>{tag}</Badge>)}</div>
        </Card>
        <Card>
          <h2>模板信息</h2>
          <div className="review-list">
            <div><span>适用年龄</span><strong>{template.ageGroup}</strong></div>
            <div><span>使用场景</span><strong>{template.useScene}</strong></div>
            <div><span>页数</span><strong>{template.pageCount} 页</strong></div>
            <div><span>来源</span><strong>{template.sourceLabel}</strong></div>
          </div>
          <p>复制后会在当前空间创建独立普通绘本副本，编辑副本不会影响市场模板。</p>
        </Card>
      </section>
      {open && (
        <Modal title="复制到当前空间" onClose={() => setOpen(false)}>
          <p>目标空间：<strong>{workspace.name}</strong></p>
          <p>空间类型：{workspace.type === "personal" ? "个人空间" : "园所空间"}</p>
          <div className="modal-actions">
            <button className="button secondary" onClick={() => setOpen(false)}>取消</button>
            <button className="button primary" type="button" disabled={copying} onClick={confirmCopy}>
              {copying ? "正在复制..." : "确认复制并打开副本"}
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
