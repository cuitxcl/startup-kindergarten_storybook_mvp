import { ArrowRight, BookOpen, CheckCircle2, Library, LockKeyhole, Sparkles, UsersRound } from "lucide-react";
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { currentSession, shouldUseApi } from "../../api/client";
import { workspaces } from "../../data/mock";
import { pickPrimaryWorkspace } from "../../utils/workspace";

const steps = [
  ["需求输入", "老师输入教学目标、年龄段和使用场景。"],
  ["故事方案", "先确认大纲，再确认角色和重复出现的配角。"],
  ["分页插图", "编辑文字、生成插图，最后导出或派生定制绘本。"],
];

const scenarios = [
  ["普通绘本", "为班级共读、规则引导和主题活动快速生成完整绘本。"],
  ["儿童定制", "基于单个孩子资料，生成独立定制副本，不覆盖母本。"],
  ["全班派生", "先做稳定母本，再按儿童资料批量生成个性化版本。"],
  ["园所市场", "沉淀优秀普通绘本，经过脱敏和审核后复用共享。"],
];

const safety = ["儿童资料按空间隔离", "分享链接可控", "园所投稿先脱敏", "公开内容需审核"];

export function HomePage() {
  const mockDemoWorkspaceId = pickPrimaryWorkspace(workspaces)?.id || workspaces[0].id;
  const [demoPath, setDemoPath] = useState(shouldUseApi ? "/app" : `/app/${mockDemoWorkspaceId}/dashboard`);
  const [marketPath, setMarketPath] = useState(shouldUseApi ? "/app" : `/app/${mockDemoWorkspaceId}/marketplace`);

  useEffect(() => {
    if (!shouldUseApi) {
      setDemoPath(`/app/${mockDemoWorkspaceId}/dashboard`);
      setMarketPath(`/app/${mockDemoWorkspaceId}/marketplace`);
      return;
    }
    currentSession()
      .then((session) => {
        const workspaceId = pickPrimaryWorkspace(session.workspaces)?.id || session.workspaces[0]?.id;
        setDemoPath(workspaceId ? `/app/${workspaceId}/dashboard` : "/app");
        setMarketPath(workspaceId ? `/app/${workspaceId}/marketplace` : "/app");
      })
      .catch(() => {
        setDemoPath("/app");
        setMarketPath("/app");
      });
  }, [mockDemoWorkspaceId]);

  return (
    <main className="home-page">
      <header className="home-nav">
        <Link className="home-brand" to="/">
          <span>K</span>
          <strong>Kindleaf</strong>
        </Link>
        <nav>
          <a href="#product">产品</a>
          <a href="#flow">生成流程</a>
          <a href="#school">园所场景</a>
          <a href="#privacy">安全隐私</a>
          <a href="#market">市场</a>
        </nav>
        <Link className="button secondary" to="/login">登录</Link>
      </header>

      <section className="home-hero" id="product">
        <div className="home-hero-copy">
          <p className="eyebrow">AI 绘本生成工作台</p>
          <h1>从教学目标到普通绘本，再到每个孩子的定制故事。</h1>
          <p>Kindleaf 帮助幼儿园老师把班级共读、规则引导、家园沟通内容沉淀为可复用的绘本资产。</p>
          <div className="home-cta">
            <Link className="button primary" to="/login">登录开始使用<ArrowRight size={16} /></Link>
            <Link className="button secondary" to={demoPath}>查看演示工作台</Link>
          </div>
        </div>
        <div className="hero-visual" aria-hidden="true">
          <div className="mock-window">
            <div className="mock-sidebar">
              <strong>园所工作台</strong>
              <span>绘本生产入口</span>
              <span>班级儿童</span>
              <span>绘本市场</span>
            </div>
            <div className="mock-main">
              <div className="mock-toolbar"><span>星星幼儿园</span><span>老师协作</span></div>
              <div className="mock-launch">
                <div><BookOpen size={22} /><strong>创建普通绘本</strong><small>从教学目标生成班级共读绘本</small></div>
                <div><UsersRound size={22} /><strong>生成定制绘本</strong><small>选择孩子并生成独立副本</small></div>
              </div>
              <div className="mock-book">
                <span>第 1 页</span>
                <strong>一起玩小汽车</strong>
                <p>老师先确认故事方案，再生成稳定角色和插图。</p>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="home-band" id="flow">
        <div className="home-section-head">
          <p className="eyebrow">How it works</p>
          <h2>先确认故事，再生成角色和页面</h2>
          <p>把生成过程拆成老师容易判断的几个阶段，减少返工。</p>
        </div>
        <div className="home-steps">
          {steps.map(([title, copy], index) => (
            <article key={title}>
              <span>{index + 1}</span>
              <h3>{title}</h3>
              <p>{copy}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="home-band soft" id="school">
        <div className="home-section-head">
          <p className="eyebrow">For schools</p>
          <h2>普通绘本是母本，定制绘本是派生</h2>
        </div>
        <div className="home-scenarios">
          {scenarios.map(([title, copy]) => (
            <article key={title}>
              <Sparkles size={20} />
              <h3>{title}</h3>
              <p>{copy}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="home-proof" id="market">
        <div>
          <p className="eyebrow">Content library</p>
          <h2>让优秀绘本成为园所内容资产</h2>
          <p>老师可以从空白生成，也可以从市场复制平台精选或园所投稿作品。园所能管理投稿、审核隐私，并逐步形成自己的内容库。</p>
          <Link className="button primary" to={marketPath}>查看绘本市场</Link>
        </div>
        <div className="proof-list">
          <div><Library size={18} /><strong>平台精选模板</strong><span>适合快速启动主题绘本</span></div>
          <div><BookOpen size={18} /><strong>园所投稿作品</strong><span>审核后可跨班级复用</span></div>
          <div><UsersRound size={18} /><strong>家长分享链接</strong><span>导出和分享边界清晰</span></div>
        </div>
      </section>

      <section className="home-band privacy" id="privacy">
        <div className="home-section-head">
          <p className="eyebrow">Safety</p>
          <h2>面向儿童资料的共享与隐私边界</h2>
        </div>
        <div className="privacy-grid">
          {safety.map((item) => (
            <div key={item}><LockKeyhole size={18} /><span>{item}</span><CheckCircle2 size={18} /></div>
          ))}
        </div>
      </section>

      <section className="home-final">
        <h2>先看演示，再把流程调成适合你园所的版本。</h2>
        <div className="home-cta">
          <Link className="button primary" to={demoPath}>进入演示</Link>
          <Link className="button secondary" to="/login">登录</Link>
        </div>
      </section>
    </main>
  );
}
