# 部署

此目录是可独立复制到服务器的部署副本，包含前后端源码、数据库迁移、依赖锁文件和 Compose 配置。所有路径均位于本目录内。

## 使用 GHCR 镜像

镜像地址为 `ghcr.io/cloudwhile/openwini:latest`（Linux amd64）。GitHub Actions 在推送到默认分支后完成检查并发布；同时提供分支、提交 SHA 和 `v*` 标签对应的镜像标签。

只需复制 `docker-compose.ghcr.yml` 和 `.env.example` 到服务器，复制 `.env.example` 为 `.env` 并填写下文环境配置，然后执行：

```bash
docker compose -f docker-compose.ghcr.yml pull
docker compose -f docker-compose.ghcr.yml up -d --remove-orphans
docker network connect --alias openwini proxy-network "$(docker compose -f docker-compose.ghcr.yml ps -q api)"
```

把 `proxy-network` 替换为实际代理网络名，外部代理转发到 `http://openwini:10180`。需要固定版本时，在 `.env` 中设置 `APP_IMAGE=ghcr.io/cloudwhile/openwini:sha-完整提交SHA`。更新时重复上述命令；若容器未重建且已连接网络，无需重复连接。

## 从源码部署

只需 Docker 和 Docker Compose。Compose 使用官方镜像编译源码，两个构建服务完成后退出，仅 Rust 服务持续运行，同时提供页面和 API。数据库使用外部 PostgreSQL。

## 仅通过 127.0.0.1:10080 访问

使用 GHCR 镜像时，在本目录执行：

```bash
docker compose -f docker-compose.ghcr.yml -f docker-compose.local.yml up -d --remove-orphans
```

使用源码编译时执行：

```bash
docker compose -f docker-compose.yml -f docker-compose.local.yml up -d --remove-orphans
```

然后通过 `http://127.0.0.1:10080` 访问，或将宿主机反向代理指向该地址。此方式无需手动连接外部 Docker 网络。后续执行更新、状态、日志等命令时，保持相同的两个 `-f` 参数。

## 启动

在本目录执行：

```bash
cp .env.example .env
```

编辑 `.env`，填写 `DB_HOST`、`DB_PORT`、`DB_USER`、`DB_PASSWORD`、`DB_NAME`、`APP_ENCRYPTION_KEY`、GitHub OAuth 配置、`ADMIN_GITHUB_LOGINS` 和正式 HTTPS `APP_BASE_URL`。数据库地址必须能从容器访问。

`APP_ENCRYPTION_KEY` 使用 `openssl rand -hex 32` 生成。GitHub OAuth 回调地址为 `{APP_BASE_URL}/api/auth/github/callback`。

```bash
docker compose config --quiet
docker compose up -d --remove-orphans
```

首次启动会下载依赖并编译，构建缓存和运行产物保存在 Docker 命名卷中。构建成功时 `api-build` 和 `web-build` 显示 `Exited (0)`，`api` 为运行状态。

## 连接反向代理

默认不映射宿主机端口、不配置自定义网络，容器内入口为 `10180`。启动后将 API 容器接入已有代理网络，把 `proxy-network` 替换为实际网络名：

```bash
docker network connect --alias openwini proxy-network "$(docker compose ps -q api)"
```

外部代理统一转发到 `http://openwini:10180`，由外部代理处理 TLS。

需要使用代理传递的客户端 IP 时，将外部代理在共享网络中的实际 IP 填入 `.env` 的 `TRUSTED_PROXY_IPS`。API 容器重建后需重新执行网络连接命令；代理容器重建后需核对其 IP。

## 查看状态

```bash
docker compose ps -a
docker compose logs --tail=100 api-build web-build api
```

`/healthz` 检查进程，`/readyz` 检查 API 和数据库就绪状态。

## 客户端与插件

平台直发账号与客户端官方开发者标识均为 `winiofficial`。插件清单使用 `minimumHostVersion`，目录 API 使用 `minHostVersion`；包内省略或为 `null` 时目录值为 `0.0.0`。兼容旧平台时可保留包内 `minHostVersion`，平台要求它与 `minimumHostVersion` 为相同的严格 SemVer 字符串。

权限标签可由开发者登记，实际能力由客户端决定；自定义标签不会增加系统权限。微信为独立官方插件 `wini_plugin_wechat`，仅默认源验证并声明 `desktop` 范围的安装可使用通用桌面代理，微信业务由插件实现，手动 ZIP 不会取得微信连接能力。默认插件源已开放，具体插件的收录状态以客户端 **插件管理 → 发现插件** 列表为准。

完整协议见[目录接口契约](https://wini-docs.abszero.cc/plugins/catalog)。

## 更新

此目录是当前源码的独立快照，根项目后续修改不会自动同步。更新时替换对应源码和锁文件，保留服务器的 `.env`、`APP_ENCRYPTION_KEY` 和 Docker 卷，在本目录重新执行：

```bash
docker compose down
docker compose up -d --remove-orphans
docker network connect --alias openwini proxy-network "$(docker compose ps -q api)"
```

在服务器上保持部署目录名或 Compose 项目名一致，以复用原有命名卷。

Telegram 独立插件使用 `wini_plugin_telegram`，该标识可登记入目录；客户端连接能力需要默认源验证。
