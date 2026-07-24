# Kindleaf

Kindleaf 是一个面向幼儿园老师和园所的 AI 绘本生成系统。它帮助老师把教学目标、班级规则、生活习惯、情绪引导和家园沟通内容，转化为可共读、可编辑、可导出、可复用的儿童绘本。

一句话目标：

> 让幼儿园老师能用 AI 快速生成普通绘本，并高效派生出适合每个孩子的定制绘本，同时帮助园所沉淀和复用优质教育内容。

## 当前状态

截至 2026-07-24，项目按“本地真实 API + PostgreSQL 可演示闭环，暂不要求生产化”口径评估，整体完成度约 95%。

已可演示：

- 老师登录与注册。
- 个人空间、园所管理员空间、园所老师空间和平台运营空间。
- 普通绘本生成：需求、故事方案、角色道具、分页图文、预览导出。
- 儿童档案维护和家长资料提交链接。
- 基于普通绘本为单个儿童生成定制绘本副本，并支持为多个儿童批量派生独立定制副本。
- PDF 导出、分享链接和匿名家庭分享页。
- 园所投稿、隐私确认、平台审核和绘本市场复制。
- PostgreSQL/SeaORM 持久化、workspace 权限、状态机、审计日志、生成任务、成本账本和存储空间限额。
- DeepSeek 文本 provider、Seedream 图片 provider 边界、脱敏、输出校验和 smoke 脚本。

当前主要缺口：

- 真实 Seedream 图片闭环需要配置 `SEEDREAM_API_KEY` 或 `ARK_API_KEY` 后运行真实图片 smoke。
- 真实生成 prompt 质量、图片审核、对象存储、邮件短信、部署监控和更完整内容安全仍属于后续生产化。

## 技术栈

- 前端：Vite + React + TypeScript + React Router + lucide-react。
- 后端：Rust + Loco + Axum + SeaORM + PostgreSQL。
- 生成：DeepSeek 文本，字节跳动 Seedream 图片，未配置真实 key 时回退结构化 mock provider。
- 文件：当前为本地 storage service，后续可替换为对象存储并保持下载 API 不变。

## 本地启动

详细交接步骤见 [docs/11-本地演示交接清单.md](docs/11-本地演示交接清单.md)。

最短启动路径：

```sh
docker compose up -d postgres
```

```sh
cd server
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/kindleaf_development \
  cargo run -p migration -- up

DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/kindleaf_development \
  cargo run --features db -- db seed

DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/kindleaf_development \
KINDLEAF_DEMO_SEED=1 \
KINDLEAF_GENERATION_PROVIDER=mock \
KINDLEAF_COST_BUDGET_WARNING_PERCENT=80 \
  cargo run --features db -- start
```

```sh
cd frontend
npm run dev:api
```

访问：

- `http://127.0.0.1:5173/`
- `http://127.0.0.1:5173/app`

演示账号：

- 邮箱：`lin@example.com`
- 密码：`demo`

## 验收命令

日常快速检查：

```sh
./scripts/check-fast.sh
```

临时库 API smoke：

```sh
./scripts/smoke-api-temp-db.sh
```

演示交接门禁：

```sh
CHECK_SMART_RUN_FULL=true ./scripts/check-smart.sh auto
```

这条门禁会运行 fast check、完整 API smoke 和浏览器 UI smoke；当前已覆盖单儿童定制、批量定制、导出、分享、园所投稿、市场复制和平台运营检查。UI smoke 启动 Chrome 时会自动尝试备用调试端口，减少本地端口偶发占用导致的误失败。

真实生成 readiness，不消耗额度：

```sh
./scripts/check-real-provider-readiness.sh --composite --allow-missing-keys
```

Seedream key 到手后再运行真实图片验收：

```sh
SEEDREAM_API_KEY=真实-key ./scripts/smoke-real-seedream-image.sh
```

## 关键文档

- [docs/06-完成度审计.md](docs/06-完成度审计.md)：当前完成度和证据。
- [docs/08-演示版冻结说明.md](docs/08-演示版冻结说明.md)：演示版冻结范围。
- [docs/09-真实生成能力接入说明.md](docs/09-真实生成能力接入说明.md)：DeepSeek/Seedream 接入、readiness 和真实 smoke。
- [docs/10-后端领域架构重构方案.md](docs/10-后端领域架构重构方案.md)：Loco 领域分层和后端重构现状。
- [docs/11-本地演示交接清单.md](docs/11-本地演示交接清单.md)：启动、账号、演示路线和交接 checklist。

## 仓库说明

`.tmp/`、`server/tmp/` 和 `frontend/dist/` 是本地验证或构建产物，不应提交。前端构建和后端生成的 PDF/PNG 会留在本地，但已经通过 `.gitignore` 排除出版本管理。
