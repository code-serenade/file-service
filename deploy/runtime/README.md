# File 服务 Linux x86_64 离线部署包

该部署包仅包含 `file-service` 的 Linux x86_64 运行镜像，不包含 Rust 编译环境、
源代码或对象存储服务。S3/MinIO/R2 由外部资源服务器提供，且 auth 服务必须先启动。

## 环境要求

- Linux x86_64
- Docker Engine
- Docker Compose plugin（`docker compose`）
- 已运行的 auth 服务
- 可访问的 S3 兼容对象存储

## 安装

```bash
tar -xzf file-service-linux-x86_64-docker.tar.gz
cd file-service-linux-x86_64-docker
vi deploy.env
./install.sh
```

默认访问地址为 `http://服务器IP:39998`。

## 配置

实施人员只修改 `deploy.env`。其中以下配置必须按实际环境填写：

- `AUTH_JWT_VERIFY_URL`：auth 的 `/internal/jwt_verify_config` 地址；默认指向同机 `39999` 端口。
- `AUTH_INTERNAL_TOKEN`：必须与 auth 包的 `INTERNAL_AUTH_TOKEN` 完全一致。
- `S3_BUCKET_IDS`：逗号分隔的逻辑存储类别列表，必须包含 `avatar,image,document,media`。
- `S3_BUCKET_<ID>`：该类别对应的真实 bucket 名称。
- `S3_BUCKET_<ID>_READ_POLICY`：读取策略，取值为 `public` 或 `private`；省略时 avatar 默认为 `public`，其他资源默认为 `private`。
- `S3_BUCKET_<ID>_KEY_PREFIX`：该类别的对象 key 前缀。
- `S3_PROVIDER`、`S3_ENDPOINT`、`S3_INTERNAL_ENDPOINT`、`S3_ADDRESSING_STYLE`、`S3_REGION`：所有 bucket 共用的默认连接配置。
- `S3_BUCKET_<ID>_PROVIDER`、`_ENDPOINT`、`_INTERNAL_ENDPOINT`、`_ADDRESSING_STYLE`、`_REGION`：仅在某个资源使用不同连接配置时填写，用于覆盖全局默认值。provider 可填 `aws`、`tencent_cos`、`minio`、`rustfs`、`cloudflare_r2` 等标识；COS 等需要虚拟主机寻址的服务使用 `virtual_host`。
- `S3_BUCKET_<ID>_ACCESS_KEY`、`_SECRET_KEY`：共享密钥文件不存在时，每个 bucket 必须分别设置这两个值。
- `S3_ACCESS_KEY`、`S3_SECRET_KEY`：同时设置时，安装器生成 `config/keys.toml` 并让所有 bucket 使用这组凭据。

所有资源共用一套 S3 服务时，只填写全局连接配置即可。个别资源需要不同服务或区域时，再填写对应的 `S3_BUCKET_<ID>_*` 覆盖项。设置全局 `S3_ACCESS_KEY`、`S3_SECRET_KEY` 时，所有 bucket 共用它们；两项都留空时，则必须为每个 bucket 设置独立的凭据。

客户端只会得到逻辑 `bucket_id`，不会提交或选择真实 bucket。`public` 类别的
`/file/sign/access` 可匿名调用；`private` 类别要求有效 JWT，并根据 JWT 中的用户 ID
校验 object-key 归属。所有上传、覆盖、删除始终要求 JWT 并校验对象归属。`USER_KEY_SALT`
在打包时随机生成，用于为每个用户生成稳定的对象 key。不要在已投入使用后修改它。

默认部署配置中的 avatar、image、document、media 都使用 `private` 读取策略。将某个类别的
`S3_BUCKET_<ID>_READ_POLICY` 改为 `public` 后，file-service 的访问签名接口允许匿名请求并
返回直链。对象存储 bucket 也必须配置为公共读，否则直链仍无法读取。该设置不会放开
上传、覆盖、删除或 bucket 列举。

每次修改配置后运行：

```bash
./restart.sh
```

常用命令：

```bash
docker compose --env-file deploy.env ps
docker compose --env-file deploy.env logs -f file
./start.sh
./restart.sh
./stop.sh
```
