# Changelog

All notable changes to IronLink will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


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
