---
name: ironlink-architecture
description: 完整项目认知 — IronLink 协议路由代理的架构、模块、数据流与开发规范
---

# IronLink 项目认知

## 项目定位

IronLink 是一个 Tauri 2 桌面应用，作为协议路由代理运行在本地。上游无关（支持 OpenAI Chat/Responses、Anthropic 协议），下游为各类 AI 终端应用（Codex Desktop、Claude Desktop 等）。

**核心链路：** 终端应用 → 请求进入 → 匹配应用协议 → 模型映射 → 选择上游供应商 → 协议转换 → 转发 → 响应转换返回

## 目录结构

### Rust 后端 (`src-tauri/src/`)

```
models/          数据结构定义
  app.rs          下游应用 AppConfig, AppInjection, MappingTarget
  profile.rs      上游供应商 RelayProfile, BackendConfig, BackendType
  chat.rs         OpenAI Chat Completions 请求/响应结构体
  responses.rs    OpenAI Responses API 请求/响应结构体
  anthropic.rs    Anthropic Messages API 请求/响应结构体
  model_info.rs   ModelInfo — Codex 模型发现接口的返回格式
  proxy_status.rs ProxyStatus, ProxyConfig

config/          持久化 + 共享运行时状态
  mod.rs          AppState (全应用状态), 代理开关逻辑, Codex config.toml 改写
  apps.rs         ~/.ironlink/apps.json 读写, 默认 Codex Desktop + Claude Desktop
  profiles.rs     ~/.ironlink/relay_profiles.json 读写

commands/        Tauri IPC 命令 (前端调用)
  apps.rs         get_apps, save_apps
  profiles.rs     get_profiles, save_profiles, activate_profile
  backend.rs      get_backend, update_backend, toggle_proxy
  status.rs       get_status, get_logs, get_proxy_config, set_proxy_config
  config.rs       get_config_file, write_config_file, get_codex_config_file, read_file_content, get/set_auto_start
  models_cmd.rs   get_models, update_models, fetch_upstream_models

api/             Axum HTTP 管理 API (替代 Tauri 的 HTTP 调用路径)
  apps.rs, backend.rs, profiles.rs, status.rs

proxy/           HTTP 代理核心
  mod.rs          handle_proxy (主入口 POST /v1/{*path}), handle_models (GET /v1/models), handle_websocket
  routing.rs      select_provider — 四步路由: 应用协议匹配 → 模型映射 → 前缀匹配 → 兜底
  auth.rs         根据 BackendType 构建认证头 (Bearer / x-api-key)
  error.rs        统一 JSON 错误响应

protocol/        协议转换层
  core/
    types.rs      规范类型: ProtocolRequest, ProtocolResponse, ContentPart, OutputItem, Usage, SseEvent
    traits.rs     InputProtocol/OutputProtocol trait, ProtocolPair 转换器
  input/
    responses.rs  ResponsesInput — 解析 Responses API 请求到 ProtocolRequest
  output/
    chat.rs       ChatOutput — ProtocolRequest/Response ↔ OpenAI Chat 格式
    anthropic.rs  AnthropicOutput — ProtocolRequest/Response ↔ Anthropic Messages 格式
    responses.rs  ResponsesOutput — ProtocolRequest/Response ↔ OpenAI Responses 格式
  sse/
    transform.rs  SseTransformStream — 包装上游 SSE 流, 转换为 Responses API SSE 事件
    chat_sse.rs   ChatSseConverter — Chat SSE → Responses SSE
    anthropic_sse.rs — Anthropic SSE → Responses SSE
    parser.rs     SSE 解析工具
  reasoning/
    mod.rs, styles.rs  推理努力度配置
  tools/
    mod.rs, apply_patch.rs, context.rs  工具调用处理
```

### 前端 (`src/`)

```
App.tsx                     主布局 — 侧边导航 + 页面路由 + 主题系统
main.tsx                    React 入口
components/pages/
  StatusPanel.tsx           概览 Dashboard — 代理状态/应用概览/供应商概览/API端点
  Applications.tsx          应用管理 — 列表/内联编辑/模型映射/配置注入/配置文件查看
  Providers.tsx             供应商管理 — 拖拽排序/预设选择/编辑/启用
  Settings.tsx              设置 — 主题风格/深色模式/字号/字体/自动启动/语言
  About.tsx                 关于页面
  LogViewer.tsx             日志查看器
  ModelList.tsx             模型列表
components/shared/
  ProviderSelector.tsx      供应商预设选择器
hooks/useApi.tsx            数据获取 hooks (useStatus, useApps, useProfiles 等)
api/index.tsx               Tauri invoke 封装 (全部后端命令的前端绑定)
i18n/index.tsx              国际化 — zh 完整 + en 空壳, 回退显示 key
appearance/store.tsx        外观状态管理 (主题/字号/字体/模式)
appearance/themeTokens.ts   6种主题配色 (石墨/极光/石板/碳素/夜曲/琥珀) × 深色/浅色
presets/index.ts            供应商预设列表 (官方/聚合/Anthropic/第三方)
types/index.tsx             前端 TypeScript 类型
index.css                   全局样式 + 工具类 + Fluent 2 卡片
```

## 核心数据结构

### 下游应用 (`AppConfig`)
```rust
AppConfig {
    id: String,                    // 唯一标识 "codex-desktop"
    name: String,                  // 显示名称 "Codex Desktop"
    protocol: String,              // 协议: "responses" | "anthropic" | "chatCompletions"
    enabled: bool,
    default_model: String,         // 默认模型
    models: Vec<String>,           // 支持的模型列表
    config_injection: Option<AppInjection>,  // 配置注入 (类型 + 路径)
    model_mappings: HashMap<String, MappingTarget>,  // 模型 → (供应商 + 上游模型)
}
```

### 上游供应商 (`RelayProfile`)
```rust
RelayProfile {
    id, provider_id, name, base_url, api_key, protocol,
    model, test_model, model_list, enabled, active,
}
```

### 共享状态 (`AppState`)
```rust
AppState {
    proxy_enabled, backend, models, relay_profiles,
    active_relay_id, apps, proxy_config, log_buffer
}
```

## 路由流程

```
Codex POST /v1/responses
  → handle_proxy()
    → 检查 proxy_enabled
    → 解析 body, 提取 model
    → routing::select_provider(apps, profiles, model, protocol)
      优先级:
        1. App 的 model_mappings (app.model_mappings[model])
        2. 供应商前缀匹配 (model = "deepseek/xxx")
        3. 供应商模型名匹配
        4. 第一个启用的供应商兜底
    → 协议转换 responses_to_upstream(body, upstream_protocol)
    → 构建认证头 build_auth_headers()
    → 转发请求到上游
    → 响应转换 upstream_to_responses(body, upstream_protocol)
    → SSE 流转换 (Chat/Anthropic SSE → Responses API SSE)
```

## 持久化文件

- `~/.ironlink/apps.json` — 下游应用配置
- `~/.ironlink/relay_profiles.json` — 上游供应商配置
- `~/.ironlink/settings.json` — 自动启动 + 代理状态
- `~/.codex/config.toml` — Codex Desktop 原生配置 (代理启用时改写)
- `~/.codex/config.toml.bak` — Codex 配置备份

## 关键常量

- 代理端口: `PROXY_PORT = 15723`
- 原始端口: `ORIG_PORT = 57321`
- Codex 默认模型: `gpt-5.5`, `gpt-5.4`, `gpt-5.4-mini`, `gpt-5.3-codex`, `gpt-5.2`

## 开发规范

- 语言: 中文 UI, Rust 注释用英文, 前端 i18n 用 `useI18n().t(key)`
- 架构风格: 协议无关, 功能分包, poc/vider/applications + providers + proxy
- 前端: React + TypeScript + Ant Design v6, Fluent 2 亚克力卡片风格
- 后端: Rust + Tauri v2 + Axum + reqwest + tokio
- 添加新协议/供应商: 在 protocol/ 下加 Input/Output impl + SSE handler, 在 protocol::mod.rs 注册
- 添加新命令: 在 commands/ 下加函数 + #[tauri::command], 在 lib.rs generate_handler! 注册

## 常见操作模式

- 新增页面: 在 components/pages/ 创建组件, 在 App.tsx navItems 和 pages 注册
- 新增 Tauri 命令: 在 commands/ 添加, 在 lib.rs 注册, 在 api/index.tsx 添加前端绑定
- 新增 i18n 键: 在 i18n/index.tsx zh 和 en 对象添加
- 新应用预设: 在 config/apps.rs default_apps() 添加 AppConfig
- 新供应商预设: 在 presets/index.ts 添加 ProviderPreset
