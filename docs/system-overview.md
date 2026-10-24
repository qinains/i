# 系统与模块概览

i 是面向代码和文件任务的 Rust agent，lninl 是项目品牌。它把模型协议、工具执行、
审批与存储分开：替换模型或添加工具，不应绕过完整工具批次校验和副作用确认。

## 三类扩展，不要混用

| 扩展 | 宿主 | 分发 | 用途 |
| --- | --- | --- | --- |
| 原生 worker | 桌面 CLI / console | 可执行程序与 JSON 清单 | 工具、hooks、provider、预算与压缩 |
| Rust/WASM plugin | 浏览器工作台 | `.hplugin.zip` | 经 host broker 读写当前工作区、插件私有 KV |
| CORS Helper | Chrome/Edge、Firefox、Safari | 浏览器扩展包 / Safari 容器工程 | 显式修正指定 API 的 CORS 响应头 |

后两者不是相同插件系统。浏览器扩展不能作为 agent 工具安装，WASM 插件也不会授予
浏览器跨域网络访问能力。

## 执行路径

桌面：输入 → agent → provider worker → 模型 → 完整工具批次校验 → 审批 → 内置工具
或 NDJSON worker → 保存会话。原生插件拥有当前用户权限，不是安全沙箱。

浏览器：输入 → WASM agent → Direct / 自建 Relay → 模型 → 校验与审批 → OPFS 文件
工具或独立 Worker 中的 WASM 插件 → IndexedDB 会话检查点。浏览器不执行 shell。

模型返回半截参数、截断结束原因或错误批次时，不执行待处理工具。不自动重放结果
不确定的工具调用。模型文字、文件、网页和插件输出均不能提升为宿主授权。

## 浏览器中的工作区与会话

- 工作区隔离文件；同一工作区的多个会话共享文件。
- 会话维护独立历史、标题、草稿、队列与选定连接。
- 持久 `/workspace` 位于 OPFS；`/tmp` 只在当前页面内存里，刷新即清空。
- 临时工作区的所有文件和会话均不持久化，重要结果应先下载。
- IndexedDB 保存会话和插件数据，不与 OPFS 构成跨存储原子事务。
- 同 origin、同工作区的协作读写使用 Web Locks；写入完成以流成功关闭为准。
- localhost 与 127.0.0.1、端口、协议或域名变化，都可能意味着不同的存储 origin。

刷新不应删除持久文件；清除站点数据或浏览器存储回收会删除。锁不能提供多文件回滚。
详见 [OPFS 边界](opfs-concurrency.md)。

## 配置与鉴权

网页首次引导默认勾选 Remember，点击保存后才写 localStorage；取消勾选则仅在内存使用。Direct 将 key
发给所配置的 API；Relay 会经过自己的服务器。两者都不会把 key 加入模型提示词。
模型、思考档位、输出额度是每连接配置，不代表服务商一定授予对应能力。

原生配置建议使用环境变量名或系统凭据引用，不把真实 key 写入仓库。网页的同源
脚本能读 localStorage；Remember 不是加密保险箱。详见 [安全报告与边界](../SECURITY.md)。

## 代码地图

| 路径 | 职责 |
| --- | --- |
| `src/agent/`、`src/workspace/` | agent 流程、内置文件工具与归档 |
| `src/browser/`、`src/platform/wasm.rs` | 浏览器 provider / WASM 桥接 |
| `plugins/runtime/` | 原生模块与常驻 worker |
| `plugins/wasm-protocol/`、`plugins/wasm-sdk/` | 浏览器插件协议与 Rust SDK |
| `plugins/wasm-package/` | 包、清单与 WASM 模块校验 |
| `web/plugins/` | 插件管理、Worker、host broker、私有存储 |
| `web/state/`、`web/sessions.js` | 会话日志与恢复 |
| `extensions/cors-helper/` | 独立浏览器 CORS 扩展 |
| `server/` | 可选同源 Relay，不是浏览器工作台的必需服务 |

## 能力与非目标

离线可启动缓存的工作台、操作持久文件、生成 ZIP；AI 仍需网络。HTML 预览使用只读
快照，不是完整后端或任意开发服务器。Safari 工程编译、WebKit viewport 测试都不等于
真实 iPhone 安装和使用验证。项目不承诺所有 API、所有浏览器后台状态或任意插件安全。
