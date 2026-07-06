# Changelog

All notable changes to IronLink will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


---
## v0.3.6 (2026-07-06)

### 协议转换全面对齐 / Full Protocol Conversion Alignment

此版本对 Codex Responses API ↔ Chat Completions ↔ Anthropic Messages API 间的全部六条协议转换路径进行了系统性修复，对齐 CodexPlusPlus 的行为规范。

#### Responses → Chat 请求转换 (`protocol/convert.rs`)
- **🟡 tool_choice 条件写入** — 仅在 `has_chat_tools` 为 true 时写入 tool_choice，避免上游在 tools 为空时收到意外字段
- **🟡 parallel_tool_calls 透传** — 请求侧与响应侧均透传该字段，保持 Codex 并行工具控制
- **🟡 normalize_chat_messages** — 确保 `assistant` 消息有 `tool_calls` 但无 `content` 时补空字符串，避免某些上游 API 拒绝请求
- **🟡 tool_choice + reasoning 交互** — reasoning 模式下强制 `tool_choice=auto`，vLLM 不会因缺字段而禁用所有工具
- **🟡 capabilities/命名空间 description 合并** — 合并 namespace 级和子工具级描述 (#17)
- **🟡 工具名 64 字符截断** — 超长工具名自动截断 + 8 位 hash 后缀防冲突 (#19)
- **🟡 Custom 工具 FREEFORM description** — 空描述时输出 `FREEFORM custom tool: {name}`，非空时追加 `This is a FREEFORM tool...` 提示 (#13)

#### 消息历史转换增强
- **🟡 input 支持 Object 类型** — 单对象输入（非数组包裹）正确处理 (#24)
- **🟡 custom_tool_call 历史包装** — 改为 `{"input": raw_text}` 格式，与 CodexPlusPlus 一致 (#26)
- **🟡 function_call_output 标准化** — JSON 可解析时重序列化，不能时包装为 `{"input": raw}` (#30)
- **🟡 tool_result 三级 fallback** — `content.tool_use_id` → `tool_call_id` → `call_id` (#28)
- **🟡 reasoning 合并去重** — 追加到现有 assistant 消息而非覆盖 (#32)
- **🟡 tool_calls 合并去重** — 按 `call_id` 合并而非直接替换 (#34)
- **🟡 orphan tool_output → user 回退** — 未匹配 call_id 的 tool result 包装为 user 消息 (#33)
- **🟡 responses_role_to_chat_role** — 支持 `latest_reminder` 角色映射 (#35)

#### Responses → Anthropic 请求转换 (`protocol/output/anthropic.rs`)
- **🟡 tools 透传** — 新增 `build_anthropic_tool()`，将 Responses tools 转为 Anthropic `input_schema` 格式
- **🟡 tool_choice 映射** — 新增 `convert_tool_choice()`，支持 `auto→{type:auto}` / `required→{type:any}` / `function→{type:tool}`
- **🟡 tool_result 格式修正** — 使用标准 `tool_result` content block
- **🟡 tool_use 格式修正** — assistant 消息中的 tool_calls 转为 `tool_use` blocks
- **🟡 thinking 配置** — 从 reasoning.effort 映射到 `{type:enabled, budget_tokens:N}`
- **🟡 stop_sequences 透传** — Responses `stop` → Anthropic `stop_sequences` 映射
- **🟡 工具名截断 + FREEFORM 描述** — 对齐 Chat 路径行为

#### Anthropic → Responses 非流式响应 (`protocol/mod.rs`)
- **🟡 stop_reason 解析** — `max_tokens`→Incomplete, `error`→Failed
- **🟡 created_at** — 从 Anthropic body 中读取
- **🟡 cache_read_input_tokens** — 透传到 Responses usage

#### 流式 SSE 转换
- **Chat SSE → Responses SSE (`protocol/sse/chat_sse.rs`)**
  - 🟡 function_call namespace 字段回填 — SSE 事件中携带 namespace，Codex 可逆向映射到原始工具
  - 🟡 custom_tool_call_input.delta 事件 — 用于 context compaction 后的工具状态恢复
  - 🟡 custom/function 两路工具判断 — 完整支持两种工具类型的输出格式
  
- **Anthropic SSE → Responses SSE (`protocol/sse/anthropic_sse.rs`)**
  - 🟡 tool_use `content_block_start` → function_call 事件
  - 🟡 `input_json_delta` → function_call_arguments.delta
  - 🟡 tool_use `content_block_stop` → function_call_arguments.done + output_item.done
  - 🟡 thinking `signature_delta` → signature 追踪，最终写入 reasoning 项 signature 字段
  - 🟡 thinking `content_block_stop` → reasoning output_item.done

#### 工具注册注入 (`protocol/tool_context.rs` + `protocol/tools/context.rs`)
- **🟡 add_namespace_tools 支持 custom/built-in 子工具** — CodeGraph 等 namespace 内自定义工具不再被过滤
- **🟡 SSE CodexToolContext 增加 namespace 查询方法** — `original_function_tool_name()` 支持 flat name → (原始名称, namespace) 逆向映射

---
## v0.3.5 (2026-07-05)

### 严重修复 / Critical Fixes
- **修复上下文压缩后 bug 修复被遗忘** — Codex 压缩上下文时丢弃了 `instructions` 和 `tools` 字段，压缩后不知道自己的工具定义和指令集，导致已修复的 bug 又被重新"修复"。现已将 `instructions` 和 `tools` 加入 `copy_original_fields`，每个 `response.completed` 事件携带这些关键字段（对齐 CodexPlusPlus 行为）
  Fix bug-fix forgotten after context compaction: `instructions` and `tools` were excluded from `copy_original_fields`, so after compaction Codex lost tool definitions and instructions. Now these fields are included in every `response.completed` event (matching CodexPlusPlus behavior)
- **添加 `custom_tool_call_input.delta` SSE 事件** — Codex 在 context compaction 后需要 `custom_tool_call_input.delta` 事件来追踪 custom tool 的参数状态，缺失该事件导致工具调用状态不完整
  Add `custom_tool_call_input.delta` SSE event: required by Codex to track custom tool argument state after context compaction

### 重大配置修正 / Critical Config Changes
- **`auto_compact_token_limit` 改为 `null`** — 之前设为 `115000`（95% of 272K），强制过早触发压缩。官方 models.json 内置模型该值均为 `null`（由 Codex 自行管理）。移除后可在 272K 满窗口后才压缩，显著减少压缩频率
  Change `auto_compact_token_limit` to `null`: previously set to 115000 (95% of 272K) forcing premature compaction. Official models.json sets this to `null` for all built-in models, letting Codex manage compaction naturally
- **移除 `effective_context_window_percent`** — 该字段非官方标准字段，移除后 Codex 使用默认 100% 有效窗口，不再因 95% 截断而提前消耗上下文
  Remove `effective_context_window_percent`: not an official field. Codex now uses 100% effective window instead of 95% truncation

### 模型模板对齐 / Model Template Alignment
- **新增 `available_in_plans`** — 从官方 models.json 同步该字段，Codex 据此判断模型在哪些计划中可用。缺失此字段可能导致模型在部分场景不被识别
  Add `available_in_plans`: synced from official models.json, determines which plans can access the model
- **移除 `use_responses_lite` 和 `auto_review_model_override`** — 这两个字段不在官方 models.json 中，属于遗留的无关字段
  Remove `use_responses_lite` and `auto_review_model_override`: extraneous fields not present in official models.json

---


## v0.3.4 (2026-07-05)

### 修复 / Bug Fixes
- **修复 SSE 流式转换中 apply_patch 子工具名映射丢失** — `CodexToolContext` 未注册 `apply_patch_add_file` 等子工具，导致 Chat API 返回的工具调用被以 `function_call` 格式发出，Codex 无法路由到 custom tool 处理器，工具执行被静默丢弃，修复被无效重试覆盖
  Fix apply_patch sub-tool name mapping lost in SSE streaming: register sub-tools (add_file, delete_file, etc.) so the SSE converter emits `custom_tool_call` with the original name instead of an unrecognizable `function_call`

### 性能优化 / Performance
- **消除 `copy_original_fields` 导致的上下文膨胀** — `response.completed` 不再复制 `tools`（含完整 schema）和 `instructions`，每轮响应减少数百字节。配合 `auto_compact_token_limit` 使上下文不再飙升至 122K
  Remove `tools` and `instructions` from `copy_original_fields`: the tool schemas (e.g. apply_patch format definition) bloated every SSE response, compounding across turns. Now responses carry only scalar passthrough fields

### 新功能 / New Features
- **新增 `auto_compact_token_limit: 115000`** — 模板和模型目录生成添加该字段，配合 `effective_context_window_percent: 95`，Codex 可在 115K token 时触发自动上下文压缩，避免无限制膨胀
  Add `auto_compact_token_limit: 115000` to model template and catalog generation; works with `effective_context_window_percent: 95` to trigger compaction at 115K tokens

### 改进 / Improvements
- **同步官方 models.json 字段** — 新增 `prefer_websockets`、`minimal_client_version`、`reasoning_summary_format`、`include_skills_usage_instructions` 及 `xhigh` 推理等级
  Sync missing fields from official models.json: add `prefer_websockets`, `minimal_client_version`, `reasoning_summary_format`, `include_skills_usage_instructions`, and `xhigh` reasoning effort level

---
## v0.3.3 (2026-07-05)

### 修复 / Bug Fixes
- **修复 context_window/max_context_window 参数未生效** — 所有 `toggle_proxy`、catalog 写入函数签名缺失 `models` 参数导致编译错误
- **修复 v0.3.3 标签打包失败** — GitHub Actions macOS runner 缺少 `create-dmg`，在 workflow 中 `brew install create-dmg`
  Fix DMG bundling on macOS CI: install `create-dmg` via Homebrew

### 新功能 / New Features
- **日志时间戳 + 自动滚动** — 日志写入自动添加 `[HH:MM:SS.mmm]` 时间戳前缀，LogViewer 自动滚动到底部，仅日志区域滚动
  Log timestamps and auto-scroll: prepend `[HH:MM:SS.mmm]` on each log line, auto-scroll log container
- **供应商模型列表支持上下文窗口配置** — 在供应商编辑页面，以表格形式展示模型列表，每行可配置 `context_window`、`max_context_window`、`input_modalities`（文本/图片/视觉标签切换）
  Per-model context window config in provider editor: table layout with editable context_window, max_context_window, and modality toggles

### 修复 / Bug Fixes
- **彻底修复 `model_providers` 写入为空 `{}`** — 写入前先 `doc.remove("model_providers")` 打破内联表循环，确保输出 `[model_providers.ironlink]` 表头格式
  Fix `model_providers` being written as empty `{}`: remove before write to break the inline-table rendering cycle, ensuring proper `[model_providers.ironlink]` table header format

### 新功能 / New Features
- **OCR 功能集成** — 在代理中拦截并识别图片中的文字内容
  OCR feature: intercept and recognize text from images in proxy

---
## v0.3.2 (2026-07-03)

### 修复 / Bug Fixes
- **`model_providers` 写入为空 `{}` 彻底修复** — `toml_edit` 链式索引 `doc["model_providers"]["ironlink"]` 自动生成内联表导致序列化丢失子段。新增 `doc["model_providers"] = toml_edit::table()` 显式创建标准表绕开该问题
  Fix `model_providers` written as empty `{}`: chained index auto-creates inline table in `toml_edit`, explicit `doc["model_providers"] = toml_edit::table()` now ensures proper serialization

---

## v0.3.1 (2026-07-03)

### 修复 / Bug Fixes
- **修复 `model_providers` 写入为空 `{}`** — 彻底解决 toml_edit 内联表渲染问题，先 `remove` 再重新创建显式表
  Fix `model_providers` being written as empty `{}`: remove and recreate table to avoid inline table rendering issues

---




## v0.3.0 (2026-07-03)

### 新功能 / New Features
- **模型映射编辑器** — 应用编辑页面新增模型映射 UI，选择供应商 → 选择模型，直观配置映射关系
  Model mapping editor in app edit form: select provider → select model, visually configure mappings
- **映射版模型目录** — 启用模型替换时，仅生成已配置映射的模型到 `ironlink-model-catalog.json`，slug 为原始模型名
  Mapped model catalog: when model replacement is enabled, only mapped models appear in catalog, slug is the original model name
- **首页应用卡片增强** — 根据实际情况显示模型列表或模型映射标签
  Enhanced app cards on overview page: display models or mapping tags based on configuration

### 修复 / Bug Fixes
- **写入 `model_providers` 丢失修复** — `doc["model_providers"]["ironlink"]` 访问前先创建显式表，避免 toml_edit 渲染为空 `{}`
  Fix `model_providers` being written as empty `{}`: ensure explicit table creation before setting nested keys
- **`model_provider` 改为 `ironlink`** — 从 `custom` 改为 `ironlink`，避免与其他供应商冲突
  Change `model_provider` value from `custom` to `ironlink` to avoid conflicts
- **禁用代理时删除模型目录** — 关闭代理后自动删除 `ironlink-model-catalog.json`，让 Codex 使用自有模型
  Delete model catalog when disabling proxy, so Codex reverts to its own models
- **应用默认模型修正** — config.toml 中的默认模型使用应用配置中的值，而非全局代理配置
  Use app-specific `default_model` for config injection instead of global proxy config
- **退出时条件性恢复** — 只在配置仍包含 IronLink 设置时才从备份恢复，避免覆盖用户手动修改的配置
  Conditional restore on exit: only restore from backup if config still contains IronLink proxy settings
- **查看配置路径修正** — `get_app_config_files` 使用与实际写入一致的路径函数
  Fix config viewer paths: use same path functions as actual write operations
- **模型选择器去重** — 上游模型下拉列表使用 `Set` 去重
  Deduplicate upstream model options using `Set`
- **映射目录移除 context_window** — 映射版 catalog 不写入硬编码上下文窗口，让 Codex 使用自有默认值
  Remove `context_window` from mapped catalog entries, let Codex use its own defaults

---


## v0.1.0 (2026-07-03)

### 新功能 / New Features
- **Multi-Provider Proxy Gateway** — 支持 DeepSeek / OpenAI / Anthropic / Google Gemini / OpenCode 等多供应商聚合代理，统一转发到 Codex Desktop
  Multi-provider proxy gateway with unified forwarding for DeepSeek, OpenAI, Anthropic, Google Gemini, OpenCode and more
- **模型替换功能** — 应用设置中可启用模型替换，替换后使用官方模型原名，仅替换显示名称；未配置的模型不展示在目录中
  Model replacement: when enabled, uses original model slugs with user-defined display names; unconfigured models excluded from catalog
- **供应商测试** — 前端点击测试连接通过 Tauri 后端发起请求，避免 CORS 限制
  Provider test: test connection via Tauri backend to bypass CORS restrictions
- **应用配置管理** — 图形化编辑应用配置 JSON，支持 CodeMirror 编辑器 + 表单化配置面板
  App Configuration UI: CodeMirror-powered JSON editor with form-based configuration panel
- **启动画面** — Tauri v2 原生启动屏，消除加载白屏
  Splash screen with Tauri v2 native window
- **异常处理与优雅关闭** — 全局错误边界 + 应用关闭时清理后端代理进程
  Global error boundary + graceful cleanup of proxy backend on app shutdown

### 改进 / Improvements
- **OpenAI 兼容路由** — 所有供应商统一使用 `/v1/chat/completions` 路由，兼容 Codex 标准请求格式
  Unified `/v1/chat/completions` routing for all providers, compatible with Codex standard request format
- **工具调用过滤** — 自动过滤 Codex 内置工具（`codex_` 前缀），仅转发用户自定义工具到上游 API
  Auto-filter Codex built-in tools (`codex_` prefix), forward only user-defined tools to upstream API

### 修复 / Bug Fixes
- **CORS 跨域问题** — 供应商测试连接从前端 `fetch` 迁移到 Tauri 后端 `test_provider_connection` 命令
  Provider test connection moved from browser `fetch` to Tauri backend command
- **Chat 工具参数类型** — 确保工具 `parameters` 的 `type` 始终为 `object`
  Ensure tool `parameters.type` is always `object`
- **模型消息格式** — `model_messages` 字段格式修正为对象而非数组
  Fixed `model_messages` field format (object instead of array)
- **模型目录模板** — 补充缺失字段，对齐官方 Codex 模型目录格式
  Updated model catalog template with missing fields aligned to official format
- **代理进程管理** — 应用关闭时正确终止后端代理进程
  Properly terminate proxy backend process on app shutdown

---


## v0.0.1 (Pre-release)

### 初始版本 / Initial Release
- 项目脚手架搭建（Tauri v2 + React + Rust Axum）
  Project scaffolding with Tauri v2 + React + Rust Axum
- 基础供应商管理 CRUD
  Basic provider CRUD management
- 基础应用配置读写
  Basic app configuration read/write
- 代理转发核心逻辑
  Core proxy forwarding logic
