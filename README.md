# Rust File Service Axum Server

## 离线部署

Linux x86_64 离线部署包默认端口为 `39998`。构建前需要在本机准备：

```bash
docker pull --platform linux/amd64 rust:1.94-alpine3.22
make build-docker-release
```

构建脚本不会拉取镜像；它使用本机 Cargo 缓存并禁用构建网络。生成的
`dist/file-service-linux-x86_64-docker.tar.gz` 不包含对象存储服务。部署时需
在唯一的 `deploy.env` 中填写 S3 凭据，并配置与 auth 服务相同的内部 token。

上传已生成的离线包：

```bash
make upload-docker-bundle
```

脚本固定上传到 `law@106.52.127.134:~/tmp/`。
