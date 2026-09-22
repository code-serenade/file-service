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
- `AUTH_AVATAR_PROVIDER_URL`：auth 的 `/internal/avatar_provider` 地址；file-service 会在启动后注册头像公开 URL，并持续重试。
- `AUTH_INTERNAL_TOKEN`：必须与 auth 包的 `INTERNAL_AUTH_TOKEN` 完全一致。
- `S3_ENDPOINT`：浏览器访问和预签名 URL 使用的对象存储地址。
- `S3_INTERNAL_ENDPOINT`：file 服务自身执行 HEAD/PUT 使用的地址；可与 `S3_ENDPOINT` 相同。
- `S3_ADDRESSING_STYLE`：默认 `path`。腾讯 COS 使用 `virtual_host`，并将 endpoint 配置为区域地址。
- `S3_ACCESS_KEY`、`S3_SECRET_KEY`：对象存储访问凭据。
- `S3_BUCKET_IDS`：逗号分隔的逻辑存储类别列表，必须包含 `avatar,image,document,media`。
- `S3_BUCKET_<ID>`：该类别对应的真实 bucket 名称。
- `S3_BUCKET_<ID>_READ_POLICY`：读取策略，取值为 `public` 或 `private`。
- `S3_BUCKET_<ID>_KEY_PREFIX`：该类别的对象 key 前缀。

客户端只会得到逻辑 `bucket_id`，不会提交或选择真实 bucket。所有写入、覆盖、删除
仍由 file-service 使用 JWT 与对象归属进行校验；`READ_POLICY` 只影响读取。`USER_KEY_SALT`
在打包时随机生成，用于为每个用户生成稳定的对象 key。不要在已投入使用后修改它。

默认部署配置中的 avatar、image、document 都使用 `private` 读取策略。将某个类别的
`S3_BUCKET_<ID>_READ_POLICY` 改为 `public` 仅表示对象存储可匿名读取；不会放开上传、
覆盖、删除或 bucket 列举。

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
