# 第一次连接 AI：只需准备三项信息

不需要写代码，也不需要安装 Node。打开 [浏览器工作台](https://i.lninl.com/app/)，进入「设置 → 模型连接」。

**先选一家你能注册、付款和访问的模型服务商。本站不出售 API Key，也不要求你提交给我们。API 使用费由服务商收取；聊天产品的会员通常不等于 API 额度。**

## 1. 去官方平台创建 API Key

API Key 是调用模型的私密凭证，类似一把可撤销的钥匙，**不是账号密码**。
只在服务商官方网站创建，不要购买来路不明的共享密钥。

| 服务商 | 去哪里获取 | 在本应用选择的协议 |
|---|---|---|
| DeepSeek | [官方开放平台](https://platform.deepseek.com/) → API Keys → 创建密钥 | OpenAI 兼容（Chat Completions） |
| 阿里云百炼（按量计费） | 按[官方获取 API Key 教程](https://help.aliyun.com/zh/model-studio/get-api-key)，选择自己的地域和业务空间 | OpenAI 兼容（Chat Completions） |
| OpenAI | [官方 API Keys 页面](https://platform.openai.com/api-keys)创建项目密钥 | OpenAI（Responses） |
| Anthropic / Claude | [官方 API Keys 页面](https://platform.claude.com/settings/keys)创建密钥 | Anthropic（Messages） |

按官方提示完成开通、额度或支付设置。密钥有时只完整显示一次，请自行安全保存。
建议使用专用、可撤销的密钥，在服务商控制台设置合适的消费限制。
服务可用地区、身份要求、价格和权限以服务商为准。

## 2. 回到设置，填 URL、模型和密钥

三个字段各有用途：

- **API URL**：调用地址，不是聊天网页地址，也不是控制台地址。
- **模型 / Model**：API 使用的模型 ID，不是随便起的名字，也不一定是网页上的展示名。
- **API 密钥 / API key**：刚才创建的完整密钥。只粘贴在密钥输入框，不放在 URL、消息或截图里。

### 可以对照填写的例子

| 服务商 | API URL | 模型 / Model |
|---|---|---|
| DeepSeek | `https://api.deepseek.com` | `deepseek-flash` |
| 百炼按量计费，北京地域 | `https://dashscope.aliyuncs.com/compatible-mode/v1` | `qwen-plus`，须确认当前账号可调用 |
| OpenAI | `https://api.openai.com/v1` | 从自己的平台可用模型列表复制支持 Responses 和工具调用的模型 ID |
| Anthropic | `https://api.anthropic.com` | 从自己的平台可用模型列表复制支持 Messages 和工具调用的 Claude 模型 ID |

以上 URL **可直接填到本应用**，不用手动拼接路径。应用会按所选协议补齐
`/chat/completions`、`/responses` 或 `/v1/messages`；已经是完整接口地址的 URL 也支持，
不会重复追加。不要把 `https://i.lninl.com/app/` 填成模型 API URL。

**百炼套餐不要混用：** 上表是按量计费的北京地域示例。
Coding Plan 使用专属密钥及 `https://coding.dashscope.aliyuncs.com/v1`，模型 ID 从该套餐支持列表复制，
不能拿按量计费的 Key、其他地域 Key 或其他套餐 Key 混用。
自定义服务或代理请复制其文档的 URL、协议与模型 ID，不要套用别家的设置。

「连接方式」先保留「直连」，「思考模式」先保留「接口默认」。不必先理解高级设置。
同一密钥不应随意绑定到陌生域名；切换服务商时同时核对 URL 和密钥。

## 3. 测试、保存，开始第一个任务

1. 点击「测试连接」。这会发送一次小请求，可能产生少量 API 费用；不是免费额度查询。
2. 测试成功后点击「保存更改」。私人设备可勾选「在此设备上记住连接配置和密钥」，下次不用再填。
3. 关闭设置，试试：**“在 /workspace 创建一个简洁的 index.html，介绍我的个人项目。”**
4. 如果出现修改审批，检查路径和内容，再点击「仅允许本次」。随后可预览 HTML、下载 ZIP。

保存连接或刷新页面不会自动发送模型请求。不要为了排查连续点击测试。
记住连接使用当前浏览器的 localStorage，不是加密保险箱；同源脚本或有权限的浏览器扩展可能读取。
浏览器内置 WASM 插件没有读取连接配置的接口，但你主动传给插件的内容对插件可见。
共享设备不建议记住密钥。详情见[安全说明](../SECURITY.md)。

## 失败时看这里

| 看到的提示 | 先做什么 |
|---|---|
| URL 无效 / Invalid URL | 填上面的完整 `https://` 地址，不用聊天网页、控制台地址，不把 Key 放进 URL |
| 401 / 403 | 检查 Key 是否完整、未失效，并与服务商、地域、套餐和 URL 对应；不要公开发送 Key |
| 模型不存在 / 无权限 | 从自己的平台复制模型 ID，核对大小写、账号权限和工具调用支持 |
| 余额不足 / 429 | 在服务商控制台检查额度、余额和速率限制，等待后重试；本站不能补充 API 额度 |
| 400 / 输出上限不支持 | 根据该模型文档调整「最大输出 tokens」，不要把上下文窗口大小当作输出上限 |
| Failed to fetch / CORS | 地址正确也可能是浏览器跨域限制；不是每次都代表密钥错误 |

**CORS 怎么办？** 优先使用支持浏览器跨域的 API，或由你自行部署可信中继。
桌面 Chrome / Edge 可按[扩展指南](cors-extension.md)安装 CORS 助手，开启时阅读权限提示。
它不能修复真实请求的认证错误或余额不足。iPhone 不能照搬桌面扩展方案，PWA 也不绕过 CORS。
不要通过关闭整个浏览器的安全机制解决问题。

## 官方参考

示例核对日期：2026-10-26。套餐、入口、模型和可用地区会变化，以下官方文档为准：

- [DeepSeek 首次调用 API](https://api-docs.deepseek.com/zh-cn/)
- [百炼 API Key](https://help.aliyun.com/zh/model-studio/get-api-key)、[Base URL 与套餐对应关系](https://help.aliyun.com/zh/model-studio/base-url)
- [OpenAI API 快速开始](https://developers.openai.com/api/docs/quickstart)
- [Anthropic API 概览](https://platform.claude.com/docs/en/api/overview)
