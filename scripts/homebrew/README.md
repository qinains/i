# Homebrew Tap 设置指南

## 概述

这个文件提供了创建 Homebrew Tap 仓库的模板和步骤。

## 创建 Homebrew Tap 仓库

### 步骤 1：创建仓库

```bash
# 使用 GitHub CLI
gh repo create homebrew-i --public --clone
cd homebrew-i
```

### 步骤 2：添加 Formula

复制 `i.rb` 到仓库根目录：

```bash
cp ../scripts/homebrew/i.rb ./i.rb
```

### 步骤 3：更新 SHA256

从 GitHub Release 获取实际的 SHA256：

```bash
# 下载归档
curl -L -o i.tar.gz https://github.com/qinains/i/releases/download/v0.1.0/i-0.1.0-aarch64-apple-darwin-min-size.tar.gz

# 计算 SHA256
shasum -a 256 i.tar.gz
```

更新 `i.rb` 中的 `sha256` 字段。

### 步骤 4：提交并推送

```bash
git add i.rb
git commit -m "Add i formula v0.1.0"
git push origin main
```

### 步骤 5：测试安装

```bash
# 添加 tap
brew tap qinains/i

# 安装
brew install i

# 验证
i --version
```

## 支持多平台

更新 `i.rb` 以支持多平台：

```ruby
class I < Formula
  desc "Plugin-first coding agent with Rust core"
  homepage "https://github.com/qinains/i"
  version "0.1.0"
  license "MIT"
  head "https://github.com/qinains/i.git", branch: "main"

  depends_on "rust" => :build

  if OS.mac? && Hardware::CPU.arm?
    url "https://github.com/qinains/i/releases/download/v0.1.0/i-0.1.0-aarch64-apple-darwin-min-size.tar.gz"
    sha256 "MAC_ARM64_SHA256"
  elsif OS.mac? && Hardware::CPU.intel?
    url "https://github.com/qinains/i/releases/download/v0.1.0/i-0.1.0-x86_64-apple-darwin-min-size.tar.gz"
    sha256 "MAC_X64_SHA256"
  elsif OS.linux? && Hardware::CPU.arm? && Hardware::CPU.is_64_bit?
    url "https://github.com/qinains/i/releases/download/v0.1.0/i-0.1.0-aarch64-unknown-linux-gnu-min-size.tar.gz"
    sha256 "LINUX_ARM64_SHA256"
  else
    url "https://github.com/qinains/i/releases/download/v0.1.0/i-0.1.0-x86_64-unknown-linux-gnu-min-size.tar.gz"
    sha256 "LINUX_X64_SHA256"
  end

  def install
    bin.install "bin/i"
  end

  test do
    system "#{bin}/i", "--version"
  end
end
```

## 自动化更新

可以创建一个 GitHub Action 来自动更新 Formula：

```yaml
# .github/workflows/update-formula.yml
name: Update Homebrew Formula

on:
  release:
    types: [published]

jobs:
  update:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Update Formula
        run: |
          # 获取最新的 SHA256
          # 更新 i.rb
          # 提交更改
```

## 发布到 homebrew-core

如果想要更广泛的分发，可以提交到官方的 homebrew-core：

1. Fork `Homebrew/homebrew-core`
2. 添加 Formula 到 `Formula/i.rb`
3. 创建 PR
4. 通过审核后会被合并

注意：homebrew-core 有更严格的审核标准。

## 用户安装

```bash
# 方法 1：使用 Tap
brew tap qinains/i
brew install i

# 方法 2：直接安装（如果使用 homebrew-core）
brew install i
```

## 维护

- 新版本发布时更新 `url` 和 `sha256`
- 测试多平台安装
- 保持 Formula 与上游同步
