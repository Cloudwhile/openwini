# WindowsIsland 插件开放平台

WindowsIsland 桌面客户端的插件目录网站。开发者在此提交插件，管理员审核后进入公开目录；客户端从平台拉取目录完成安装与更新。插件本体永远放在开发者自己的
公开 GitHub 仓库 Release 附件里，平台不托管插件代码，只托管元数据、审核状态和索引。

> 人工审核主要用于减少接口滥用，不代表插件安全、可靠或适合你的设备。外部插件由客户端隔离运行；权限标签不自动授予系统能力，已授权的后台联网仍需谨慎核查。
> 下载和使用插件前，请自行核对来源、权限、代码与 Release 内容。

## 技术栈与布局

- 前端 `apps/web`：React + Vite + TypeScript + shadcn/ui（base-nova 风格）+ Tailwind CSS 4
- 组件包 `packages/ui`：工作区共享 shadcn 组件
- 后端 `crates/api`：Rust + axum + tokio，sqlx（运行时查询）
- 数据库 `migrations/`：PostgreSQL 迁移（随仓库走，启动时自动执行）
- 邮件：lettre（STARTTLS / SSL / 不加密，失败重试 3 次、指数退避）
- `docker-compose.yml`：使用官方镜像编译后端与前端，不依赖 Dockerfile，不包含 PostgreSQL。`api-build`、`web-build` 编译成功后退出，仅 `api` 持续运行，由 Rust 服务统一提供页面和 API。

## 快速开始（Docker 部署）

可直接复制 `deploy/` 目录到服务器，按其中的 `README.md` 部署；该目录包含完整部署源码，不依赖父目录。

GitHub Actions 会将通过检查的应用发布到 `ghcr.io/cloudwhile/openwini:latest`（Linux amd64，包含前端和 API）。使用 `deploy/docker-compose.ghcr.yml` 可直接拉取运行；该配置同样不映射宿主机端口，网络通过 `docker network connect` 手动连接。详细命令见 `deploy/README.md`。

数据库使用你自己的外部 PostgreSQL 实例（托管云数据库、自建服务器均可）。

```bash
# 1) 配置环境
cp .env.example .env
# 编辑 .env：填写外部数据库（DB_HOST/DB_PORT/DB_USER/DB_PASSWORD/DB_NAME）、
# APP_ENCRYPTION_KEY、GitHub OAuth、ADMIN_GITHUB_LOGINS、正式 HTTPS APP_BASE_URL

# 2) 启动（自动编译源码，迁移在后端启动时自动执行）
docker compose up -d --remove-orphans
# 默认不向宿主机发布端口；启动后手动接入代理网络
```

Compose 默认不向宿主机发布任何端口，使用 Compose 自动创建的默认网络，不指定网络名、网段或固定 IP。部署后使用 `docker network connect` 将 API 容器接入已有的反向代理网络（把 `proxy-network` 替换为实际网络名）：

```bash
docker network connect --alias openwini proxy-network "$(docker compose ps -q api)"
```

反向代理将站点流量转发到 `http://openwini:10180`，Rust 服务直接提供前端静态文件、浏览器路由回退和 `/api` 接口。API 容器重建后需重新执行网络连接命令。正式域名应填入 GitHub OAuth 的回调地址：`{APP_BASE_URL}/api/auth/github/callback`。如需宿主机访问，可在本地 Compose 覆盖文件中显式配置 `ports`。`--remove-orphans` 会清理旧配置遗留的 Web 容器。

需要使用代理传递的客户端 IP 时，在 `.env` 的 `TRUSTED_PROXY_IPS` 中填写外部反向代理在共享网络中的实际 IP，并运行 `docker compose up -d --no-deps --force-recreate api` 应用配置，再重新连接代理网络。反向代理容器重建后需重新核对该地址；留空时 API 使用直连地址进行限流和访问记录。

首次启动会下载编译依赖并生成生产产物，耗时取决于网络和机器性能。源码以只读方式挂载，依赖缓存、后端编译缓存和运行产物保存在 Docker 命名卷中，不使用宿主机的 `node_modules` 或 `target`。构建失败会阻止对应运行服务启动，可通过 `docker compose logs api-build web-build` 查看原因。

更新源码后重新编译并启动：

```bash
docker compose down
docker compose up -d
```

`down` 会保留缓存和产物卷，后续编译可复用依赖；两个构建服务正常完成后的状态为 `Exited (0)`。

## 本地开发

```bash
# 自备 PostgreSQL（本机安装或外部实例均可），在 .env 中配置 DB_* 分键变量

# 后端：启动时自动加载仓库根目录的 .env（已导出的环境变量优先级更高）
cargo run -p wini-api                            # 监听 :8080，自动迁移

# 前端
npm install
npm run dev --workspace apps/web                 # 监听 :3000，/api 代理到 :8080
```

测试与构建：

```bash
cargo test            # 后端单元测试（契约校验、加密、AI 输出解析）
cargo build --release
npm run build --workspace apps/web   # 前端类型检查 + 生产构建
```

## 环境变量

| 变量 | 必填 | 说明 |
| --- | --- | --- |
| `DB_HOST` / `DB_USER` / `DB_PASSWORD` / `DB_NAME` | 是 | PostgreSQL 分键连接配置；也可用 `DATABASE_URL` |
| `APP_ENCRYPTION_KEY` | 是 | 32 字节十六进制（`openssl rand -hex 32`），加密 GitHub Token / SMTP 密码 / AI Key |
| `GITHUB_CLIENT_ID` / `GITHUB_CLIENT_SECRET` | 是（OAuth） | GitHub OAuth App；回调地址 `{APP_BASE_URL}/api/auth/github/callback`；范围固定 `read:user user:email public_repo` |
| `ADMIN_GITHUB_LOGINS` | 建议 | 逗号分隔的引导管理员 GitHub 登录名（只授予、不收回） |
| `OFFICIAL_GITHUB_LOGIN` | 否 | **唯一**绑定直发账号（`winiofficial`，system 权限）的 GitHub 用户名；该账号登录后自动获得管理员权限，可走直发流程。绑定会解除该 GitHub 账号与普通开发者行的关联 |
| `APP_BASE_URL` | 否 | 前端基地址，默认 `http://localhost:5173`（用于回跳与邮件链接） |
| `BIND_ADDR` | 否 | 本地默认 `0.0.0.0:8080`；Compose 使用 `0.0.0.0:10180` |
| `WEB_DIST_DIR` | 否 | 前端生产产物目录；默认留空，仅提供 API。Compose 挂载产物卷到 `/web` 并自动启用 |
| `COOKIE_SECURE` | 否 | 本地 http 调试设 `false`；生产必须 `true` |
| `APP_ENV` | 否 | 本地使用 `development`；Compose 自动使用 `production` |
| `TRUSTED_PROXY_IPS` | 否 | 逗号分隔的受信代理 IP；默认留空。填写外部反向代理在共享网络中的实际 IP |
| `RUST_LOG` | 否 | 日志级别 |

## 首个管理员引导流程

1. 在 GitHub 创建 OAuth App，回调 `{APP_BASE_URL}/api/auth/github/callback`，把 Client ID/Secret 写入环境变量。
2. 在 `.env` / 部署环境中设置 `ADMIN_GITHUB_LOGINS=alice,bob`——可配置一位或多位；
   管理员可以审批自己的申请（全程有审计记录），多管理员仍建议以便互相复核。
3. 名单中的 GitHub 账号首次通过 GitHub 登录后自动成为管理员（`is_admin`）。
4. 此后管理员可在「开发者管理」中互相授予或收回管理员权限；环境变量只负责引导授予。

## 宿主与插件契约

平台直发账号与宿主官方开发者标识统一为 **`winiofficial`**。官方身份由客户端验证默认插件源的安装记录，包内自声明或手动导入不授予官方能力。直发内容仍需接受启用条款与联网须知。

包内最低宿主版本使用 `minimumHostVersion`，目录 API 使用 `minHostVersion`；省略包内字段或设为 `null` 时，目录值为 `0.0.0`。两者使用严格 SemVer。兼容旧平台的包可同时保留 `minHostVersion`，但平台要求它与 `minimumHostVersion` 为相同字符串，不能只提供旧字段。现有微信发布包保留两个一致字段。

平台预置 `notifications`、`settings`、`wechat`、`network`、`process`、`filesystem`、`native`、`automation` 权限标签；开发者也可登记不重名的自定义标签。平台校验标签已登记，宿主根据实际支持的服务决定授予能力；自定义标签不增加系统能力。`process` 与 `automation` 仅为经来源和范围核验的桌面适配器提供宿主代理，仍不授予直接系统访问；`filesystem`、`native` 不向外部插件扩权。旧 `wechat` 标签只保留历史兼容性，不再授予专用监听。

微信是独立官方插件：`wini_plugin_wechat`，开发者 `winiofficial`，仓库 `https://github.com/WinIOfficial/wini_plugin_wechat`，声明 `notifications`、`settings`、`automation` 与 `process`，并在包内 `desktop` 声明客户端范围。微信窗口解析、消息判断、免打扰和头像由插件实现；宿主只向默认源验证的该适配器提供有界通用桌面与模块代理。默认插件源已开放，微信是否收录以客户端 **插件管理 → 发现插件** 列表为准；GitHub Release 与平台收录分别生效。

完整协议见[目录接口契约](https://wini-docs.abszero.cc/plugins/catalog)。

## 宿主公开 API（契约摘要）

基地址默认使用部署配置中的 `APP_BASE_URL`，管理员可在后台覆盖（`catalog_base_url`）。
升级时会清除旧版占位地址，保留已配置的自定义地址。
索引路径固定：

- `GET /api/sources` → 顶层数组，源条目字段：`developerId`、`developerName`、`repositoryUrl`、
  `pluginsUrl`（恒等于 `{base}/api/sources/{developerId}/plugins`）
- `GET /api/sources/{developerId}/plugins` → 该开发者全部 published 插件，条目字段：
  `id`、`name`、`version`、`developerId`、`repositoryUrl`、`downloadUrl`、`sha256`、
  `minHostVersion`、`contractVersion`、`permissions`、`description`，以及可选的 `network` 联网须知

规则：公开只读、限流、带缓存头；响应体 UTF-8 JSON、顶层数组、单响应 ≤4 MiB；
空数组合法；永远直接返回 200 + 最新 JSON（无 304、无 3xx）；`downloadUrl` 指向 GitHub
Release 附件原地址（`https://github.com/{owner}/{repo}/releases/download/{tag}/{file}.zip`），
不做平台中转。未审核、已停止内容绝不进入索引；`updates_halted` 只出现冻结版本。
字段约束在入库与出索引时双重校验（严格 SemVer 无前导零、权限标签需在平台注册、
SHA-256 小写 64 位十六进制等，详见 `crates/api/src/validation.rs` 及其单元测试）。

**同一开发者的已发布插件必须来自同一个 GitHub 仓库**：宿主源契约要求插件条目的
`repositoryUrl` 与所属源条目逐字一致，因此平台在提交与发布两个环节都会校验并阻断。

## 核心流程

- **开发者**：GitHub 登录 → 首次登录自动以 GitHub 用户名生成 developerId（小写化、连字符转
  下划线、数字开头补 u 前缀、重名自动加 _1/_2 后缀，此后不可改）→ 从本人仓库列表选择仓库
  （平台校验：公开仓库 / owner 或 admin 权限 / Release 存在 ZIP 附件）→ 选 Release 版本 →
  平台自动读取仓库 `manifest.json` 逐项比对（标识、名称、版本、描述、权限、minimumHostVersion、
  合约大版本），并计算 ZIP 的 SHA-256 → 提交进入待审。
- **状态机**：`draft → pending → published`；退回 `rejected`（可改后重提）；`cancelled`
  （draft/pending 可取消，可基于原内容重新申请）；管理员可 `stopped`（立即移出索引，可恢复）、
  `updates_halted`（冻结版本保留在索引、新版本不进索引，可恢复）。新版本是挂在已发布插件下的
  独立申请，退回不影响已发布旧版本。停止期间排队的待审申请会被自动退回；停止更新期间排队的新
  版本申请须先恢复更新再审核发布。
- **管理员**：审核队列（按提交时间列出待审申请，允许审批自己的申请，队列中对本人申请有标注）、
  插件管理（含直发账号管理）、开发者管理（封禁 → 名下已发布插件同步停止、解封后需逐个恢复）、
  SMTP / AI / 站点配置、审计日志（只增不改，数据库触发器禁止 UPDATE/DELETE）。所有状态变更与审计
  写入同一事务。
- **直发账号 `winiofficial`**：系统内置开发者身份，管理员以该身份直接创建/发布/停止/恢复插件，
  不走申请队列，但每一步记审计。这只是后台操作路径，不代表插件安全或可靠。管理员个人名义的插件走普通
  开发者流程，与直发账号互不相干。
- **SMTP 通知**：首次 GitHub 注册欢迎邮件、异常登录提醒、申请回执、审核通过、退回（含原因）、
  新版本发布、停止、停止更新、恢复、管理员新申请待审。开发者可在用户菜单设置邮件语言；中文之外
  的偏好统一使用英文。模板只含插件/版本/开发者/原因/链接，绝不包含密钥或 token。每封邮件正文与
  后台发送记录都带事件 ID，审核、退回、停止、恢复等审计事件会复用对应 ID。发送失败重试 3 次
  （指数退避），最终失败记入发送记录并在后台标红，不阻塞审核。
- **AI 审核**：默认关闭。开启后每次申请自动运行，输入仅含清单全文、权限列表、仓库文件列表、
  README、Release 信息（不含任何密钥）；输出风险标记 + 三选一建议（通过/退回/需人工复核），
  仅作建议、永不自动决策；失败时标注「需人工复核」不阻塞。开发者只能看到公开建议（风险标记），
  内部推理不展示。

## 部署

1. 准备域名、TLS 终止代理容器和外部 PostgreSQL，确认反向代理使用的 Docker 网络。
2. 复制 `.env.example` 为 `.env`，配置数据库、应用加密密钥、GitHub OAuth 与正式 HTTPS `APP_BASE_URL`；回调地址为 `{APP_BASE_URL}/api/auth/github/callback`。
3. 按「快速开始」运行 `docker compose up -d --remove-orphans`，再使用 `docker network connect` 将 API 容器接入代理网络，代理转发到 `http://openwini:10180`。检查 `docker compose ps -a` 的服务状态和 `docker compose logs api` 的启动日志。迁移自动执行；`winiofficial` 身份与默认站点配置自动初始化。
4. 管理员登录后在「站点设置」配置：目录对外基地址（正式域名）、SMTP、（可选）AI 审核。
5. 核对客户端使用相同的官方身份、最低宿主版本映射和权限声明，确认已发布条目可从默认插件源读取。

TLS 代理若传递客户端来源地址，应只为代理实际来源配置受信来源，并由其转发客户端地址。`/readyz` 用于检查 API 和数据库就绪状态；`/healthz` 用于进程存活检查。

## 安全要点

- GitHub OAuth 全程校验 state；会话使用 HttpOnly（+Secure）Cookie + CSRF Token（写请求必须带
  `X-CSRF-Token`）；所有管理接口校验 `is_admin`；开发者接口校验资源归属（只能操作自己名下插件，
  管理员的个人插件同样受限）。
- 密钥类配置（GitHub Token、SMTP 密码、AI Key）用应用级密钥 AES-256-GCM 加密存库，日志与前端
  永不出现，密钥轮换不影响已发布插件。
- 公开 API 与站内接口按 IP 限流；随机数全部使用系统安全随机源；审计日志只增不改。
