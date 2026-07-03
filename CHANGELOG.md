# Changelog

All notable changes to IronLink will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---
## v0.1.1 (2026-07-03)

### 修复 / Bug Fixes
- **保留 context_window 字段** — 修复因删除 `context_window`/`max_context_window` 导致 Codex 不压缩对话、payload 超限的问题
  Retain `context_window`/`max_context_window` in model catalog; fixes Codex conversation truncation
- **修复 CI 构建** — 添加 `@tauri-apps/cli` 依赖；`npm run tauri build` 使用 `--` 分隔符传递 target 参数
  Fix CI builds: add `@tauri-apps/cli` dependency, use `--` separator for target args

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
## v0.1.1 (2026-07-03)

### 修复 / Bug Fixes
- **保留 context_window 字段** — 修复因删除 `context_window`/`max_context_window` 导致 Codex 不压缩对话、payload 超限的问题
  Retain `context_window`/`max_context_window` in model catalog; fixes Codex conversation truncation
- **修复 CI 构建** — 添加 `@tauri-apps/cli` 依赖；`npm run tauri build` 使用 `--` 分隔符传递 target 参数
  Fix CI builds: add `@tauri-apps/cli` dependency, use `--` separator for target args

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
