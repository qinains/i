# 打包工具使用指南

## 概述

项目现在使用纯 Rust 实现的打包工具，不再依赖 Python。这样可以：
- 跨平台支持（macOS、Linux、Windows）
- 统一技术栈（只需要 Rust 工具链）
- 更好的性能和可维护性

## 快速开始

### 1. 编译打包工具

```bash
cargo build --features packaging --bin i-package
```

### 2. 使用打包脚本

```bash
# 使用默认配置（min-size profile）
./scripts/package.sh

# 或者直接使用 Rust 工具
./target/debug/i-package
```

## 命令行参数

```
Options:
      --profile <PROFILE>    构建配置 [default: min-size] 
                             [possible values: release, min-size]
      --skip-build           跳过 cargo build（使用现有二进制）
      --outline              额外的 AArch64 LLVM 优化（需要构建）
  -h, --help                 显示帮助信息
```

## 自动平台检测

打包工具会根据目标平台自动选择压缩格式：

| 平台 | 压缩格式 | 文件扩展名 |
|------|---------|-----------|
| macOS | tar.gz | `.tar.gz` |
| Linux | tar.gz | `.tar.gz` |
| Windows | zip | `.zip` |

## 使用示例

### 示例 1：完整打包（构建 + 打包）

```bash
./scripts/package.sh --profile min-size
```

这会：
1. 使用 `cargo build --features bundled --bin i --profile min-size` 构建
2. 创建包含所有必需文件的归档
3. 生成 SHA256 校验和

### 示例 2：使用现有二进制打包

```bash
# 先手动构建
cargo build --features bundled --bin i --profile min-size

# 然后打包（跳过构建步骤）
./scripts/package.sh --skip-build
```

### 示例 3：AArch64 优化构建

```bash
# 仅限 macOS arm64
./scripts/package.sh --profile min-size --outline
```

## 产物结构

打包完成后，`dist/` 目录会包含：

```
dist/
├── i-0.1.0-aarch64-apple-darwin-min-size/
│   ├── bin/i                    # 主二进制文件（bundled 模式）
│   ├── config.example.json      # 配置示例
│   ├── README.txt               # 使用说明
│   ├── start.sh                 # 启动脚本
│   ├── logo.svg                 # Logo
│   ├── LICENSE-globset.txt      # 第三方许可证
│   ├── BUILD.json               # 构建元数据
│   └── SHA256SUMS               # 文件校验和
├── i-0.1.0-aarch64-apple-darwin-min-size.tar.gz
└── i-0.1.0-aarch64-apple-darwin-min-size.tar.gz.sha256
```

## 验证打包结果

使用验证脚本检查归档文件：

```bash
./scripts/check_package.sh dist/i-*.tar.gz
```

验证内容包括：
- SHA256SUMS 文件校验
- BUILD.json 格式检查（layout 必须是 "bundled"）
- 二进制文件数量（应该只有 1 个）
- 二进制文件可执行性
- 归档文件校验和

## 发布到包管理器

### Homebrew (macOS/Linux)

1. **创建 Homebrew Tap**：
   ```bash
   # 创建 tap 仓库
   gh repo create homebrew-i --public --clone
   cd homebrew-i
   
   # 创建 Formula
   cat > i.rb <<'EOF'
   class I < Formula
     desc "Plugin-first coding agent"
     homepage "https://github.com/qinains/i"
     version "0.1.0"
     
     if OS.mac? && Hardware::CPU.arm?
       url "https://github.com/qinains/i/releases/download/v0.1.0/i-0.1.0-aarch64-apple-darwin-min-size.tar.gz"
       sha256 "YOUR_SHA256_HERE"
     elsif OS.mac? && Hardware::CPU.intel?
       url "https://github.com/qinains/i/releases/download/v0.1.0/i-0.1.0-x86_64-apple-darwin-min-size.tar.gz"
       sha256 "YOUR_SHA256_HERE"
     elsif OS.linux? && Hardware::CPU.arm? && Hardware::CPU.is_64_bit?
       url "https://github.com/qinains/i/releases/download/v0.1.0/i-0.1.0-aarch64-unknown-linux-gnu-min-size.tar.gz"
       sha256 "YOUR_SHA256_HERE"
     else
       url "https://github.com/qinains/i/releases/download/v0.1.0/i-0.1.0-x86_64-unknown-linux-gnu-min-size.tar.gz"
       sha256 "YOUR_SHA256_HERE"
     end
     
     def install
       bin.install "bin/i"
     end
   end
   EOF
   
   git add i.rb && git commit -m "Add i formula" && git push
   ```

2. **用户安装**：
   ```bash
   brew tap qinains/i
   brew install i
   ```

### Cargo (跨平台)

```bash
# 发布到 crates.io
cargo publish

# 用户安装
cargo install i
```

### GitHub Releases

1. **创建 Release**：
   ```bash
   # 打包所有平台
   ./scripts/package.sh --profile min-size
   
   # 上传到 GitHub Release
   gh release create v0.1.0 \
     dist/i-*.tar.gz \
     dist/i-*.tar.gz.sha256 \
     --title "v0.1.0" \
     --notes "Release notes..."
   ```

2. **自动化发布**（GitHub Actions）：
   ```yaml
   # .github/workflows/release.yml
   name: Release
   
   on:
     push:
       tags:
         - 'v*'
   
   jobs:
     release:
       strategy:
         matrix:
           include:
             - os: macos-latest
               target: aarch64-apple-darwin
             - os: macos-latest
               target: x86_64-apple-darwin
             - os: ubuntu-latest
               target: x86_64-unknown-linux-gnu
             - os: windows-latest
               target: x86_64-pc-windows-msvc
       
       runs-on: ${{ matrix.os }}
       steps:
         - uses: actions/checkout@v4
         
         - name: Build
           run: cargo build --features bundled,packaging --bin i --bin i-package --profile min-size --target ${{ matrix.target }}
         
         - name: Package
           run: ./target/${{ matrix.target }}/min-size/i-package --skip-build
         
         - name: Upload to Release
           uses: softprops/action-gh-release@v1
           with:
             files: dist/*
   ```

### Winget (Windows)

1. **创建 manifest**：
   ```yaml
   # Manifests/q/qinains/i/0.1.0/qinains.i.yaml
   PackageIdentifier: qinains.i
   PackageVersion: 0.1.0
   PackageName: i
   Publisher: qinains
   License: MIT
   Installers:
     - Architecture: x64
       InstallerType: zip
       InstallerUrl: https://github.com/qinains/i/releases/download/v0.1.0/i-0.1.0-x86_64-pc-windows-msvc-min-size.zip
       InstallerSha256: YOUR_SHA256_HERE
   ```

2. **提交到 winget-pkgs**：
   ```bash
   # Fork winget-pkgs
   # 提交 manifest
   # 创建 PR
   ```

## 技术细节

### Bundled 模式

打包工具使用 `--features bundled` 构建，这意味着：
- `i-plugins` 的所有功能都静态链接到 `i` 二进制中
- 只需要一个可执行文件
- `i` 通过 `i --runtime <module>` 调用内置的插件功能
- 不需要独立的 `i-plugins` 文件

### 压缩格式

- **Unix 系统**：使用 `.tar.gz`（gzip 压缩的 tar 归档）
- **Windows**：使用 `.zip`（更好的 Windows 兼容性）

### 校验和

每个归档文件都会生成对应的 `.sha256` 文件，用于验证完整性。

## 与 Python 版本的对比

| 特性 | Rust 版本 | Python 版本 |
|------|----------|------------|
| 依赖 | Rust 工具链 | Python 3 + 标准库 |
| 跨平台 | ✅ 完整支持 | ✅ 支持 |
| 性能 | 更快 | 较慢 |
| 维护成本 | 统一技术栈 | 额外依赖 |
| 功能 | 完整 | 完整 |

Python 版本（`scripts/package.py`）仍然保留，可以作为备选方案。

## 故障排除

### 问题：找不到打包工具

```bash
# 重新编译
cargo build --features packaging --bin i-package
```

### 问题：归档文件无法解压

确保使用正确的方式解压：

```bash
# Unix (tar.gz)
tar xzf archive.tar.gz

# Windows (zip)
# 使用 Windows 资源管理器或 7-Zip
```

### 问题：校验和不匹配

重新生成校验和：

```bash
# 删除旧的校验和文件
rm dist/*.sha256

# 重新打包
./scripts/package.sh --skip-build
```

## 更多信息

- 项目 README: `../README.md`
- 构建配置: `../Cargo.toml`
- 打包工具源码: `../src/bin/package.rs`
