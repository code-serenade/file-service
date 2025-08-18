#!/bin/bash
set -e

# 获取项目名（当前目录名）
PROJECT_NAME=$(basename "$PWD")

# Rust 编译后的二进制名（把 - 替换成 _）
BINARY_NAME="${PROJECT_NAME//-/_}"

# 编译 release
echo "🚀 Building project: $PROJECT_NAME ..."
cargo build --release

# 安装路径
TARGET_DIR="$HOME/bin/$PROJECT_NAME"

# 创建目标目录
mkdir -p "$TARGET_DIR"

# 复制 config 目录（如果存在）
if [ -d "config" ]; then
  echo "📂 Copying config/ to $TARGET_DIR ..."
  cp -r config "$TARGET_DIR/"
else
  echo "⚠️ No config directory found, skipping."
fi

# 找到编译好的可执行文件
BINARY="target/release/$BINARY_NAME"
if [ -f "$BINARY" ]; then
  echo "📦 Installing binary ($BINARY_NAME) to $TARGET_DIR ..."
  cp "$BINARY" "$TARGET_DIR/"
else
  echo "❌ Binary not found: $BINARY"
  exit 1
fi

# 清理 target 目录
echo "🧹 Removing target/ directory ..."
rm -rf target

echo "✅ Done! Files installed to: $TARGET_DIR"