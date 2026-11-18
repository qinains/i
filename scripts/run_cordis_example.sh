#!/bin/bash
# Cordis 示例运行脚本

echo "=== Cordis 模块系统示例 ==="
echo ""

# 运行完整示例
echo "运行完整示例..."
cargo test --test cordis_semantics -- --nocapture

echo ""
echo "=== 示例完成 ==="
