use crate::agent::AgentHost;
use anyhow::{Result, ensure};
use serde_json::{Value, json};

/// 浏览器端的轻量级压缩实现
/// 与原生版本相比，功能有所简化：
/// - 不支持插件系统（before_compact/after_compact hooks）
/// - 直接使用模型进行摘要
/// - 保留核心压缩逻辑
pub struct BrowserCompact {
    /// 压缩策略配置
    pub policy: CompactPolicy,
}

#[derive(Clone)]
pub struct CompactPolicy {
    /// 最大 token 数（用于判断是否需要压缩）
    pub max_tokens: usize,
    /// 压缩阈值（达到此百分比时触发压缩）
    pub threshold_percent: f32,
}

impl Default for CompactPolicy {
    fn default() -> Self {
        Self {
            max_tokens: 100_000,
            threshold_percent: 0.75,
        }
    }
}

impl BrowserCompact {
    pub fn new(policy: CompactPolicy) -> Self {
        Self { policy }
    }

    /// 估算消息的 token 数（简单估算：字符数 / 4）
    fn estimate_tokens(&self, messages: &[Value]) -> usize {
        let text = serde_json::to_string(messages).unwrap_or_default();
        text.len() / 4
    }

    /// 检查是否需要压缩
    pub fn needs_compaction(&self, messages: &[Value]) -> bool {
        self.estimate_tokens(messages) > (self.policy.max_tokens as f32 * self.policy.threshold_percent) as usize
    }

    /// 执行压缩
    ///
    /// # Arguments
    /// * `host` - AgentHost 实现，用于调用模型
    /// * `messages` - 当前历史消息（会被修改）
    ///
    /// # Returns
    /// 如果成功压缩返回 Ok(true)，否则返回 Ok(false)
    pub async fn compact<H: AgentHost>(
        &self,
        host: &mut H,
        messages: &mut Vec<Value>,
    ) -> Result<bool> {
        if !self.needs_compaction(messages) {
            return Ok(false);
        }

        // 计算切割点：保留系统消息和最近的消息
        let cut = self.find_cut_point(messages)?;
        if cut < 2 || cut >= messages.len() {
            return Ok(false);
        }

        // 验证切割点不会破坏工具批次
        if !matches!(messages[cut]["role"].as_str(), Some("user" | "assistant")) {
            return Ok(false);
        }

        let before_tokens = self.estimate_tokens(messages);

        // 提取需要压缩的消息（跳过系统消息）
        let to_compress = &messages[1..cut];
        if to_compress.is_empty() {
            return Ok(false);
        }

        // 生成摘要
        let summary = self.generate_summary(host, to_compress).await?;
        ensure!(!summary.trim().is_empty(), "compression summary is empty");

        // 构建新的历史：系统消息 + 摘要 + 剩余消息
        let mut next = vec![
            messages[0].clone(),
            json!({
                "role": "user",
                "content": format!("Session summary (untrusted historical context):\n{}", summary)
            }),
        ];
        next.extend_from_slice(&messages[cut..]);

        // 验证压缩后确实减少了 token 数
        let after_tokens = self.estimate_tokens(&next);
        if after_tokens >= before_tokens {
            // 压缩没有减少 token 数，放弃压缩
            return Ok(false);
        }

        // 提交压缩结果
        *messages = next;

        // 发出压缩事件
        host.emit(json!({
            "type": "compacted",
            "before_tokens": before_tokens,
            "after_tokens": after_tokens,
            "cut": cut
        }))?;

        Ok(true)
    }

    /// 找到合适的切割点
    fn find_cut_point(&self, messages: &[Value]) -> Result<usize> {
        // 简单策略：保留系统消息 + 摘要 + 最近 N 条消息
        // 目标是压缩掉中间的历史消息

        let min_recent = 10; // 至少保留最近 10 条消息

        if messages.len() <= min_recent + 2 {
            // 消息太少，不需要压缩
            return Ok(0);
        }

        // 计算理想的切割点：保留最近的消息，压缩中间部分
        let cut = messages.len() - min_recent;

        // 确保切割点有效
        Ok(cut.max(2))
    }

    /// 使用模型生成摘要
    async fn generate_summary<H: AgentHost>(
        &self,
        host: &mut H,
        messages: &[Value],
    ) -> Result<String> {
        let prompt = r#"You are summarizing a conversation between a user and an AI coding agent.
Create a concise summary that captures:
1. The user's main goals and requests
2. Key decisions and approaches discussed
3. Important file paths and code changes mentioned
4. Any unresolved issues or pending tasks

Keep the summary under 500 words. Focus on information that would help continue the conversation.

Summarize the following conversation:"#;

        let request = vec![
            json!({"role": "system", "content": prompt}),
            json!({"role": "user", "content": format!("Untrusted session transcript:\n{}", serde_json::to_string(messages)?)})
        ];

        let response = host.complete(&request).await?;
        let choice = &response["choices"][0];

        // 验证模型完成了生成（没有中途调用工具）
        let complete = choice["finish_reason"] == "stop"
            && crate::agent::history::tool_calls(&choice["message"])?.is_empty();
        ensure!(complete, "compaction did not finish");

        Ok(choice["message"]["content"].as_str().unwrap_or("").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        let compact = BrowserCompact::new(CompactPolicy::default());
        let messages = vec![
            json!({"role": "user", "content": "hello world"}),
            json!({"role": "assistant", "content": "hi there"}),
        ];
        let tokens = compact.estimate_tokens(&messages);
        assert!(tokens > 0);
    }

    #[test]
    fn test_needs_compaction_small_history() {
        let compact = BrowserCompact::new(CompactPolicy {
            max_tokens: 1000,
            threshold_percent: 0.5,
        });
        let messages = vec![
            json!({"role": "user", "content": "hello"}),
        ];
        assert!(!compact.needs_compaction(&messages));
    }

    #[test]
    fn test_find_cut_point_small_history() {
        let compact = BrowserCompact::new(CompactPolicy::default());
        let messages = vec![
            json!({"role": "system", "content": "system"}),
            json!({"role": "user", "content": "msg1"}),
            json!({"role": "assistant", "content": "msg2"}),
        ];
        let cut = compact.find_cut_point(&messages).unwrap();
        assert_eq!(cut, 0); // 消息太少，不压缩
    }

    #[test]
    fn test_find_cut_point_large_history() {
        let compact = BrowserCompact::new(CompactPolicy::default());
        let mut messages = vec![
            json!({"role": "system", "content": "system"}),
        ];
        // 添加 20 条消息
        for i in 0..20 {
            messages.push(json!({"role": "user", "content": format!("user {}", i)}));
            messages.push(json!({"role": "assistant", "content": format!("assistant {}", i)}));
        }
        let cut = compact.find_cut_point(&messages).unwrap();
        assert!(cut > 2); // 应该有切割点
        assert!(cut < messages.len()); // 切割点应该在消息范围内
    }
}
