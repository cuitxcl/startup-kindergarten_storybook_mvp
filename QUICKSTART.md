# Quick Start

## 1. 前端启动

默认前端会优先连接本地后端 `127.0.0.1:8080`。如果你想看 mock 原型，可以显式切到 mock 模式。

```sh
cd frontend
npm install
npm run dev -- --host 127.0.0.1
```

默认访问：

- `http://127.0.0.1:5173/`
- `http://127.0.0.1:5173/app/school-1/dashboard`

## 2. 数据库

真实 API 演示依赖 PostgreSQL。先启动数据库：

```sh
docker compose up -d postgres
```

首次启动或 schema 变化后运行 migration：

```sh
cd server
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/kindleaf_development \
  cargo run -p migration -- up
```

写入演示数据：

```sh
cd server
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/kindleaf_development \
  cargo run --features db -- db seed
```

seed 会创建演示用户、个人空间、园所空间、管理员、老师、班级、儿童、绘本、投稿和市场模板。它是幂等的，可以在本地开发时重复执行。

schema 变化后，可以用临时库验证 migration 能否回滚并重放：

```sh
./scripts/check-migrations.sh
```

这个脚本会创建临时 PostgreSQL 库，执行 `up -> reset -> up`，再连续运行两次 `db seed`，结束后自动删除临时库。默认使用 `kindleaf-postgres` 容器和 `127.0.0.1:55432`。

本地或试点前可以先用脚本备份当前数据库：

```sh
./scripts/backup-postgres.sh
```

默认会从 `kindleaf-postgres` 容器备份 `kindleaf_development` 到 `.tmp/backups/*.dump`，并用 `pg_restore -l` 验证备份文件可读。恢复脚本默认恢复到一个新的时间戳数据库，不会覆盖已有库：

```sh
./scripts/restore-postgres.sh .tmp/backups/kindleaf_development-YYYYMMDD-HHMMSS.dump
```

如果需要验证备份和恢复链路，可以运行临时库自测：

```sh
./scripts/check-smart.sh backup-restore
```

这个检查会创建临时源库，完成 migration 和 seed，备份后恢复到另一个临时库，并比较 `users/workspaces/storybooks` 这些核心表的记录数。

## 3. 后端 API

启动 Loco 后端，默认监听 `127.0.0.1:8080`：

```sh
cd server
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/kindleaf_development \
  cargo run --features db -- start
```

如果希望后端启动时自动补齐演示数据，可以在开发或测试环境打开：

```sh
KINDLEAF_DEMO_SEED=1
```

生成服务当前默认使用结构化 mock provider，不需要真实 AI 密钥：

```sh
KINDLEAF_GENERATION_PROVIDER=mock
KINDLEAF_COST_BUDGET_WARNING_PERCENT=80
```

如果要验证真实文本生成，可以显式切到 DeepSeek。没有配置时默认仍使用 mock provider：

```sh
KINDLEAF_GENERATION_PROVIDER=deepseek
DEEPSEEK_API_KEY=your-api-key
DEEPSEEK_ENDPOINT_PATH=/chat/completions
DEEPSEEK_MODEL=deepseek-v4-flash
```

真实 provider smoke 和 readiness 脚本会自动读取仓库根目录或 `server/` 下的 `.env.local`、`.env`；命令行里临时传入的环境变量优先级更高。这些文件已被 `.gitignore` 忽略，适合放本地真实 key。

DeepSeek provider 当前只覆盖文本任务：故事方案、角色设定、分页图文和定制方案；图片生成默认使用字节跳动 Seedream，未配置密钥时回退到 mock 结果。

导出 PDF 和生成插图默认写入本地 `tmp/exports` 与 `tmp/generated-images`。试点或部署环境可以通过以下变量改到持久化目录，API 下载地址保持不变：

```sh
KINDLEAF_STORAGE_ROOT=/var/lib/kindleaf
# 如需分别指定，也可以覆盖：
KINDLEAF_EXPORTS_DIR=/var/lib/kindleaf/exports
KINDLEAF_GENERATED_IMAGES_DIR=/var/lib/kindleaf/generated-images
KINDLEAF_EXPORT_MAX_BYTES=52428800
KINDLEAF_GENERATED_IMAGE_MAX_BYTES=15728640
```

默认 PDF 上限为 50MB，生成插图 PNG 上限为 15MB；对应变量设为 `0` 时关闭该上限。

如果要在不消耗真实 API 的情况下验证 DeepSeek provider 链路，可以运行本地伪 DeepSeek smoke：

```sh
./scripts/smoke-generation-provider.sh
```

这个脚本会创建临时 PostgreSQL 库，启动本地 fake DeepSeek 服务，以 `KINDLEAF_GENERATION_PROVIDER=deepseek` 启动后端，依次创建 `storybook_plan`、`storybook_roles`、`storybook_pages` 和 `customization_plan` 生成任务，并确认任务输出来自 `deepseek` provider。其中角色和分页任务还会验证生成结果已写入对应绘本。

注意：provider smoke 默认使用 Loco test 后端端口 `8081`，请顺序运行，不要和其他 provider smoke 并行执行。

如果要验证字节跳动 Seedream 图片 provider 链路，可以运行本地伪 Seedream 图片 smoke：

```sh
./scripts/smoke-image-provider.sh
```

这个脚本会创建临时 PostgreSQL 库，启动本地 fake Seedream 图片服务，以 `KINDLEAF_GENERATION_PROVIDER=seedream` 启动后端，创建单页插图任务，并确认任务输出来自 `seedream` provider、图片已写入本地文件且只能通过登录态 API 下载。

注意：provider smoke 默认使用 Loco test 后端端口 `8081`，请顺序运行，不要和其他 provider smoke 并行执行。

如果要验证“文本 + 图片”组合 provider，可以运行：

```sh
./scripts/smoke-composite-provider.sh
```

这个脚本会同时启动本地 fake DeepSeek 和 fake Seedream 图片服务，不显式指定 `KINDLEAF_GENERATION_PROVIDER`，只通过 `DEEPSEEK_API_KEY` 和 `SEEDREAM_API_KEY` 触发组合 provider，确认后端显示 `deepseek+seedream`，并在同一个服务进程里分别完成文本任务和插图任务。

注意：组合 provider smoke 同样使用 Loco test 后端端口 `8081`，请顺序运行，不要和其他 provider smoke 并行执行。

也可以直接运行 provider smoke 总入口，按顺序验证文本、图片和组合模式：

```sh
./scripts/smoke-providers.sh
```

真实 provider 也有总入口。它会自动读取 `.env.local` / `.env`，先做不消耗额度的 readiness 检查，再按本机已配置的 key 顺序运行真实 DeepSeek、真实 Seedream 和真实组合 smoke；明显占位 key 会被当作未就绪，不会触发真实调用；Seedream 图片相关 smoke 会验证 PNG 下载、未登录/跨空间下载拒绝和生成成本账本；缺少某个 provider key 时会跳过对应真实调用：

```sh
./scripts/smoke-real-providers.sh
```

如果你希望某项缺 key 时直接失败，可以显式要求：

```sh
RUN_DEEPSEEK=required RUN_SEEDREAM=required RUN_COMPOSITE=required ./scripts/smoke-real-providers.sh
```

同一组强制真实链路也可以通过 smart check 的短入口运行，适合试点交接前作为“真实生成必须全部跑通”的门禁：

```sh
./scripts/check-smart.sh real-required
```

真实 provider smoke 会消耗额度。真实 smoke 脚本会自动先运行不调用模型的前置检查；配置真实密钥前后，也可以单独运行这个检查，它会从 `.env.local` / `.env` 读取本地 key，并打印 provider 配置、PostgreSQL、端口、storage 目录、文件大小上限和文件名安全规则：

```sh
./scripts/check-real-provider-readiness.sh --composite
```

当前只是检查本地环境、不想因为缺少密钥失败时，可以加：

```sh
./scripts/check-real-provider-readiness.sh --composite --allow-missing-keys
```

试点部署前还可以运行不调用模型的总 readiness。默认模式只打印风险提醒；`--strict` 会把本地地址、临时 storage、mock provider、缺少真实生成 key、明显占位密钥、provider endpoint/model 配置错误、未配置预算上限等试点风险作为失败处理：

```sh
./scripts/check-trial-readiness.sh
./scripts/check-trial-readiness.sh --strict
```

试点配置可以从 `.env.trial.example` 开始复制到 `.env.local`，再替换真实域名、数据库、`KINDLEAF_AUTH_TOKEN_SECRET`、DeepSeek key、Seedream/ARK key、持久化 storage 和预算上限。配置完成后建议直接跑硬门禁：

```sh
cp .env.trial.example .env.local
openssl rand -base64 48
./scripts/check-smart.sh trial-strict
```

如果使用模板里的默认 storage 目录，需要先在部署机器上创建并授权给后端运行用户：

```sh
sudo mkdir -p /var/lib/kindleaf/{exports,generated-images}
sudo chown -R <kindleaf-runtime-user>:<kindleaf-runtime-group> /var/lib/kindleaf
```

如果要验证后端运营端 readiness 在试点配置齐全时会返回 `ready=true`，可以运行不调用真实模型的专项 smoke。它会创建临时库、seed 演示运营账号，然后用假 DeepSeek/Seedream key、HTTPS `APP_HOST`、持久化 storage 和预算上限启动后端，只检查 `/api/operator/readiness`：

```sh
./scripts/smoke-operator-readiness.sh
```

如果采用 DeepSeek + Seedream 组合模式，可以把 `KINDLEAF_GENERATION_PROVIDER` 显式留空，并同时配置 `DEEPSEEK_API_KEY` 与 `SEEDREAM_API_KEY` 或 `ARK_API_KEY`；trial readiness 会显示 `auto-composite`。

2026-07-21 最近一次本地 readiness 结果：脚本已能从 `.env` 读取本地 DeepSeek key；DeepSeek/Seedream endpoint、Docker/PostgreSQL、测试端口、storage 写入、文件大小上限和文件名安全规则均 OK；DeepSeek endpoint path 已支持通过 `DEEPSEEK_ENDPOINT_PATH` 覆盖；真实 provider 总入口已能自动运行真实 DeepSeek 文本 smoke，并在缺少 Seedream/ARK key 时跳过真实图片和组合 smoke；真实 DeepSeek 文本 smoke 已通过故事方案、角色、分页、定制方案、写回、成本账本和敏感输入脱敏审计验收；真实 Seedream 图片 smoke 已验证缺 key 时会在 readiness 阶段退出，不会误调用模型，补齐 key 后会同时验 PNG 下载、下载权限和成本账本；真实组合 provider 试运行仍需要补齐 `SEEDREAM_API_KEY` 或 `ARK_API_KEY`。

常用验证：

```sh
curl http://127.0.0.1:8080/api/health
curl -H "Authorization: Bearer dev-token" http://127.0.0.1:8080/api/auth/me
cargo run --features db -- routes
```

不启动服务、不碰数据库的快速检查：

```sh
./scripts/check-fast.sh
```

这个脚本会检查 shell/Node 脚本语法、Docker Compose 配置、前端 mock/API strict guard、前端 API 模式构建、后端格式和后端单测，适合小步改动后快速确认没有破坏主线。完整 API 或 UI 行为仍以 `smoke-api-temp-db.sh`、`smoke-all.sh` 和 provider smoke 为准。

日常推进建议优先使用智能检查调度器：

```sh
./scripts/check-smart.sh
```

它会根据当前改动文件选择最短验证路径：

- 纯文档改动：不跑代码检查。
- 前端或普通后端小改动：先跑 `check-fast.sh`。
- 后端 API 或配置改动：追加临时库 API smoke。
- migration 或核心模型改动：追加 migration 回滚/重放检查。
- 生成 provider 改动：追加 provider smoke。
- 试点配置改动：追加 trial readiness 和 trial-positive 正向自测，不调用外部模型。
- 运营端 readiness 改动：可显式运行 operator readiness 正向 smoke，不调用外部模型。
- 前端 UI 改动：默认只提示全量 UI smoke，避免每次小改动都跑最慢链路。

阶段验收或准备交接时再强制跑完整浏览器闭环：

```sh
CHECK_SMART_RUN_FULL=true ./scripts/check-smart.sh
./scripts/check-smart.sh demo
```

也可以显式选择验证范围：

```sh
./scripts/check-smart.sh fast
./scripts/check-smart.sh api
./scripts/check-smart.sh full
./scripts/check-smart.sh demo
./scripts/check-smart.sh migrations
./scripts/check-smart.sh backup-restore
./scripts/check-smart.sh providers
./scripts/check-smart.sh trial
./scripts/check-smart.sh trial-strict
./scripts/check-smart.sh trial-positive
./scripts/check-smart.sh operator-readiness
```

推荐的快速 API 闭环 smoke：

```sh
./scripts/smoke-api-temp-db.sh
```

这个脚本会创建临时 PostgreSQL 库，运行 migration，启动测试环境 Loco 后端，打开 `KINDLEAF_DEMO_SEED=1`，执行完整 API smoke，结束后自动停止后端并删除临时库。它适合后端迭代时频繁验证，不会污染 `kindleaf_development`。

如果需要保留临时库排查问题：

```sh
KEEP_DB=true ./scripts/smoke-api-temp-db.sh
```

已有后端进程和开发库上的完整 API smoke：

```sh
./scripts/smoke-api.sh
```

如果后端不是 `8080` 端口：

```sh
API_BASE_URL=http://127.0.0.1:8091 ./scripts/smoke-api.sh
```

这个脚本会验证登录、空间、工作台、权限边界、儿童档案、普通绘本、插图任务、定制绘本、导出、分享、市场复制、成员、班级、投稿和家长资料提交，并在结束时清理临时数据。

完整前后端演示闭环 smoke：

```sh
./scripts/smoke-all.sh
```

这个脚本会先对目标数据库执行 migration，再自动启动临时 Loco 后端和 API 模式 Vite 前端，然后依次运行 API smoke 与 UI smoke。UI smoke 会用系统 Chrome headless 检查登录、普通绘本创建、定制绘本生成、园所投稿、平台审核上架、市场复制复用、园所生成队列恢复和取消生成任务。默认端口为后端 `8111`、前端 `5178`，可通过 `API_PORT`、`FRONTEND_PORT` 覆盖。

如果希望用容器启动一套本地演示环境，可以使用 Compose 的 `app` profile。默认 `docker compose up -d postgres` 仍只启动数据库，不会构建应用镜像：

```sh
docker compose up -d postgres
docker compose --profile app run --rm migrate
docker compose --profile app up --build api frontend
```

访问：

- 前端：`http://127.0.0.1:5173/`
- 后端：`http://127.0.0.1:8080/api/health`

查看容器健康状态：

```sh
docker compose --profile app ps
```

API 容器会检查 `/api/health`，前端容器会检查 `/healthz`。如果健康状态不是 `healthy`，先查看对应容器日志：

```sh
docker compose --profile app logs api
docker compose --profile app logs frontend
```

Compose 演示默认使用 mock provider 和容器内持久化 storage volume，不会自动读取 `.env` 里的真实 DeepSeek/Seedream 密钥。若确实要把真实 provider key 注入容器，请使用 `COMPOSE_DEEPSEEK_API_KEY`、`COMPOSE_SEEDREAM_API_KEY` 或 `COMPOSE_ARK_API_KEY` 这些显式变量。

如果 `8080` 已被占用，可以用 production 配置临时换端口：

```sh
cd server
PORT=8091 APP_HOST=http://127.0.0.1:8091 \
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/kindleaf_development \
  cargo run --features db -- -e production start
```

如果你想强制看 mock 原型：

```sh
cd frontend
npm run dev:mock
```

## 4. 前端连接真实 API

后端启动后，在另一个终端启动 API 优先前端：

```sh
cd frontend
npm run dev:api
```

默认访问：

- `http://127.0.0.1:5173/`
- `http://127.0.0.1:5173/app/school-1/dashboard`
- `http://127.0.0.1:5173/app/school-1/storybooks`
- `http://127.0.0.1:5173/app/school-1/admin`

API 模式下，前端会把 `personal-1`、`school-1`、`school-2` 这些演示路由别名映射到后端真实 workspace UUID。

## 5. 构建和测试

前端：

```sh
cd frontend
npm run build
npm run build:api
npm run smoke:ui
```

后端：

```sh
cd server
cargo fmt --check
cargo check --features db
cargo test --features db
```

## 当前阶段状态

- 前端是 Vite + React + TypeScript。
- 后端是 Loco + Axum + SeaORM。
- PostgreSQL 持久化已覆盖主要演示闭环：账号、空间、工作台、儿童档案、绘本、市场、投稿、分享和导出任务。
- AI 生成已具备 provider 边界、结构化 mock 输出、DeepSeek 文本 HTTP provider 和字节跳动 Seedream 图片 provider；真实试点还需要继续收口模型密钥、配额、对象存储、邮件短信和支付。
