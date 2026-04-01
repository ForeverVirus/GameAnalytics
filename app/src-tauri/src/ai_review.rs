use crate::graph::model::*;

fn is_chinese_ui(response_language: &str) -> bool {
    !matches!(
        response_language.trim().to_ascii_lowercase().as_str(),
        "en" | "en-us" | "en-gb" | "english"
    )
}

fn output_language_instruction(response_language: &str) -> &'static str {
    if is_chinese_ui(response_language) {
        "输出要求：除 severity 字段外，category、message、suggestion 以及所有自然语言说明必须使用简体中文。severity 字段必须保持 Critical | Warning | Info | Suggestion。"
    } else {
        "Output requirement: Except for the severity field, category, message, suggestion, and all natural-language explanations must be in English. The severity field must remain Critical | Warning | Info | Suggestion."
    }
}

/// Build a prompt for AI line-level code review
pub fn build_line_review_prompt(
    file_path: &str,
    file_content: &str,
    lang: &str,
    response_language: &str,
) -> String {
    format!(
        r#"你是一位资深的游戏开发代码审查专家。请对以下 {lang} 文件进行逐行代码审查。

文件路径: {file_path}
{output_language_instruction}

请检查以下方面:
1. 潜在 Bug 和逻辑错误
2. 空指针/空引用隐患
3. 资源泄漏（未释放的资源、未取消的订阅）
4. 线程安全问题
5. 硬编码的魔法数字或字符串
6. 未处理的异常/错误
7. 性能反模式（如在 Update 循环中频繁分配内存）

请以 JSON 数组格式返回发现的问题, 每个问题包含:
- "line_number": 行号(整数)
- "line_end": 结束行号(整数, 可选)
- "severity": "Critical" | "Warning" | "Info" | "Suggestion"
- "category": 问题类别(字符串)
- "message": 问题描述(字符串)
- "suggestion": 修复建议(字符串, 可选)

只返回 JSON 数组, 不要返回其他内容。如果没有问题, 返回空数组 []。

文件内容:
```{lang}
{file_content}
```"#,
        output_language_instruction = output_language_instruction(response_language)
    )
}

/// Build a prompt for AI architecture review
pub fn build_arch_review_prompt(
    file_path: &str,
    file_content: &str,
    upstream: &[String],
    downstream: &[String],
    lang: &str,
    response_language: &str,
) -> String {
    let up_list = if upstream.is_empty() {
        "无".to_string()
    } else {
        upstream.join(", ")
    };
    let down_list = if downstream.is_empty() {
        "无".to_string()
    } else {
        downstream.join(", ")
    };

    format!(
        r#"你是一位资深的游戏架构师。请对以下 {lang} 文件进行架构层面的审查。

文件路径: {file_path}
上游依赖(被依赖): {up_list}
下游依赖(依赖的): {down_list}
{output_language_instruction}

请检查以下方面:
1. 单一职责原则 — 该文件是否承担了太多职责
2. 耦合度 — 是否与其他模块过度耦合
3. 循环依赖风险
4. 设计模式的合理性 (单例滥用、God Class 等)
5. 可测试性
6. 模块划分建议

请以 JSON 数组格式返回发现的问题, 每个问题包含:
- "severity": "Critical" | "Warning" | "Info" | "Suggestion"
- "category": 问题类别(字符串)
- "message": 问题描述(字符串)
- "suggestion": 修复建议(字符串, 可选)

只返回 JSON 数组, 不要返回其他内容。如果没有问题, 返回空数组 []。

文件内容:
```{lang}
{file_content}
```"#,
        output_language_instruction = output_language_instruction(response_language)
    )
}

/// Build a prompt for AI performance review
pub fn build_perf_review_prompt(
    file_path: &str,
    file_content: &str,
    lang: &str,
    response_language: &str,
) -> String {
    format!(
        r#"你是一位游戏性能优化专家。请对以下 {lang} 文件进行性能审查。

文件路径: {file_path}
{output_language_instruction}

请重点检查以下方面:
1. Update/FixedUpdate/LateUpdate 中的性能问题
   - 频繁的内存分配 (new, string 拼接, LINQ)
   - 不必要的 GetComponent 调用
   - 频繁的 Find 调用
2. 内存管理
   - 大型数组/列表的频繁创建销毁
   - 未使用对象池
   - 闭包捕获导致的 GC 压力
3. 渲染相关
   - 材质实例化未回收
   - 过多的 Draw Call (动态批处理障碍)
4. 物理相关
   - Raycast 滥用
   - 不当的碰撞检测层设置
5. I/O 与网络
   - 同步 I/O 阻塞主线程
   - 不必要的序列化/反序列化

请以 JSON 数组格式返回发现的问题, 每个问题包含:
- "line_number": 行号(整数, 可选)
- "line_end": 结束行号(整数, 可选)
- "severity": "Critical" | "Warning" | "Info" | "Suggestion"
- "category": 问题类别(字符串)
- "message": 问题描述(字符串)
- "suggestion": 修复建议(字符串, 可选)

只返回 JSON 数组, 不要返回其他内容。如果没有问题, 返回空数组 []。

文件内容:
```{lang}
{file_content}
```"#,
        output_language_instruction = output_language_instruction(response_language)
    )
}

/// Build a prompt for AI asset optimization suggestion
pub fn build_asset_optimization_prompt(
    metrics_json: &str,
    response_language: &str,
) -> String {
    format!(
        r#"你是一位游戏美术资源优化专家。请根据以下资源指标数据，给出优化建议。

资源指标数据(JSON):
```json
{metrics_json}
```
{output_language_instruction}

请检查以下方面:
1. 纹理
   - 分辨率是否过大 (>2048 是否必要)
   - 是否应启用 mipmap
   - 压缩格式建议
   - 估计内存是否超标
2. 音频
   - 采样率是否过高
   - 文件大小是否合理
   - 是否应使用压缩格式
3. 模型
   - 文件过大可能面数过高

请以 JSON 数组格式返回优化建议, 每个建议包含:
- "file_path": 资源路径(字符串)
- "severity": "Critical" | "Warning" | "Info" | "Suggestion"
- "category": 问题类别(字符串)
- "message": 问题描述(字符串)
- "suggestion": 修复建议(字符串)

只返回 JSON 数组, 不要返回其他内容。如果没有建议, 返回空数组 []。"#,
        output_language_instruction = output_language_instruction(response_language)
    )
}

/// Parse AI response into ReviewFindings
pub fn parse_review_response(
    raw: &str,
    review_type: ReviewType,
    file_path: &str,
    node_id: &str,
    response_language: &str,
) -> ReviewResult {
    let findings = parse_findings_from_json(raw, &review_type, file_path);
    let json_str = extract_json_array(raw);
    let is_empty_array = json_str.trim() == "[]";

    let summary = if findings.is_empty() {
        if raw.trim().is_empty() {
            if is_chinese_ui(response_language) {
                "AI 未返回任何内容".to_string()
            } else {
                "AI returned no content".to_string()
            }
        } else if is_empty_array {
            if is_chinese_ui(response_language) {
                "未发现问题".to_string()
            } else {
                "No issues found".to_string()
            }
        } else {
            if is_chinese_ui(response_language) {
                "AI 返回了内容但未能解析为结构化结果，请查看原始回复".to_string()
            } else {
                "AI returned content, but it could not be parsed into structured findings. See the raw response.".to_string()
            }
        }
    } else {
        let critical = findings.iter().filter(|f| f.severity == ReviewSeverity::Critical).count();
        let warning = findings.iter().filter(|f| f.severity == ReviewSeverity::Warning).count();
        if is_chinese_ui(response_language) {
            format!("发现 {} 个问题 ({} 严重, {} 警告)", findings.len(), critical, warning)
        } else {
            format!(
                "Found {} issues ({} critical, {} warnings)",
                findings.len(),
                critical,
                warning
            )
        }
    };

    ReviewResult {
        node_id: node_id.to_string(),
        review_type,
        findings,
        summary,
        timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        raw_response: raw.to_string(),
    }
}

fn parse_findings_from_json(
    raw: &str,
    review_type: &ReviewType,
    file_path: &str,
) -> Vec<ReviewFinding> {
    if raw.trim().is_empty() {
        return Vec::new();
    }
    // Try to extract JSON array from the response
    let json_str = extract_json_array(raw);
    let arr: Vec<serde_json::Value> = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(_) => {
            // JSON parse failed — return a fallback finding with the raw text
            return vec![ReviewFinding {
                id: format!("{}_raw", file_path.replace('/', "_")),
                review_type: review_type.clone(),
                file_path: file_path.to_string(),
                line_number: None,
                line_end: None,
                severity: ReviewSeverity::Info,
                category: "AI原始回复".to_string(),
                message: raw.to_string(),
                suggestion: None,
            }];
        }
    };

    arr.iter().enumerate().filter_map(|(i, v)| {
        let severity = match v.get("severity").and_then(|s| s.as_str()).unwrap_or("Info") {
            "Critical" => ReviewSeverity::Critical,
            "Warning" => ReviewSeverity::Warning,
            "Suggestion" => ReviewSeverity::Suggestion,
            _ => ReviewSeverity::Info,
        };

        Some(ReviewFinding {
            id: format!("{}_{}", file_path.replace('/', "_"), i),
            review_type: review_type.clone(),
            file_path: v.get("file_path").and_then(|s| s.as_str())
                .unwrap_or(file_path).to_string(),
            line_number: v.get("line_number").and_then(|n| n.as_u64()).map(|n| n as u32),
            line_end: v.get("line_end").and_then(|n| n.as_u64()).map(|n| n as u32),
            severity,
            category: v.get("category").and_then(|s| s.as_str()).unwrap_or("General").to_string(),
            message: v.get("message").and_then(|s| s.as_str()).unwrap_or("").to_string(),
            suggestion: v.get("suggestion").and_then(|s| s.as_str()).map(|s| s.to_string()),
        })
    }).collect()
}

/// Extract the first JSON array from a string, handling markdown code blocks
fn extract_json_array(raw: &str) -> String {
    // Try to find ```json ... ``` block
    if let Some(start) = raw.find("```json") {
        let after = &raw[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    if let Some(start) = raw.find("```") {
        let after = &raw[start + 3..];
        if let Some(end) = after.find("```") {
            let inner = after[..end].trim();
            if inner.starts_with('[') {
                return inner.to_string();
            }
        }
    }
    // Try to find bare JSON array
    if let Some(start) = raw.find('[') {
        if let Some(end) = raw.rfind(']') {
            return raw[start..=end].to_string();
        }
    }
    raw.to_string()
}

/// Detect programming language from file extension
pub fn detect_language(file_path: &str) -> &'static str {
    let ext = file_path.rsplit('.').next().unwrap_or("");
    match ext {
        "cs" => "csharp",
        "gd" => "gdscript",
        "js" => "javascript",
        "ts" => "typescript",
        "py" => "python",
        "lua" => "lua",
        "shader" | "cginc" | "hlsl" => "hlsl",
        "glsl" => "glsl",
        _ => "text",
    }
}
