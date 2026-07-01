# IronLink

> 多供应商 AI 代理网关 · 流量拦截、协议转换、模型映射

IronLink 是一款专为 AI 开发者打造的桌面代理工具。它通过本地拦截 Codex Desktop 的网络流量，实现多供应商聚合、协议转换、模型映射和流量管理。支持 OpenAI Responses API / Chat Completions 和 Anthropic Claude 协议的自动转换与自由切换。

---

## 功能亮点

### 🚦 流量拦截与转发
- 本地代理拦截 Codex 所有 API 请求，无需修改 DNS 或系统代理
- 自动将 Codex 模型名映射到任意上游供应商的模型
- 支持 `http://` 和 `ws://` 协议代理

### 🔄 协议自动转换
- **Responses API ↔ Chat Completions**: OpenAI 新一代 API 与标准 Chat API 双向互转
- **Responses API ↔ Anthropic Claude**: 支持与 Anthropic Messages API 互转
- 思考链（reasoning/thinking）内容保留并独立传输

### 🏢 多供应商聚合
- 同时配置多个 AI 供应商，自动路由请求
- 供应商预设支持：DeepSeek、OpenCode、Agnes 等
- 通过预设模板快速添加新供应商

### 🔀 模型映射
- 将 Codex 内置的 5 个模型（GPT-5.5/5.4/5.4-Mini/5.3-Codex/5.2）映射到任意上游模型
- 可视化映射管理界面，实时编辑
- 映射规则持久化到本地文件

### 🔌 WebSocket 代理
- 正向代理 Messages API WebSocket 连接
- 自动重写模型字段，保持映射一致性
- 双向消息透明转发

### 📊 实时状态面板
- 代理启停、端口状态、后端类型一目了然
- 实时代理日志查看
- 供应商模型列表可视化

---

## 快速开始

### 前置条件

- Node.js 20+
- Rust 1.85+
- Tauri CLI

### 安装与运行

```bash
# 克隆项目
git clone https://github.com/scpuny/ironlink.git
cd ironlink

# 安装前端依赖
npm install

# 开发模式（前端热更新 + Rust 后端）
npm run dev

# 纯 Rust 后端开发
npm run dev:rust

# 生产构建
npm run build
npm run build:rust
```

### 使用流程

1. 启动 IronLink，在 **供应商** 页面添加 AI 供应商（或从预设中选择）
2. 在 **模型映射** 页面将 Codex 模型映射到上游模型
3. 在 **概览** 页面点击 **启用** 按钮
4. 重启 Codex Desktop
5. Codex 的所有 API 请求将通过 IronLink 转发到上游供应商

---

## 项目结构

```
ironlink/
├── src/                          # 前端 (React + TypeScript + Ant Design)
│   ├── App.tsx                   # 应用主入口
│   ├── api/                      # Tauri IPC 调用封装
│   ├── components/pages/         # 页面组件
│   │   ├── StatusPanel.tsx       # 概览/状态面板
│   │   ├── Providers.tsx         # 供应商管理
│   │   ├── ModelMappings.tsx     # 模型映射管理
│   │   ├── ConfigEditor.tsx      # Codex 配置文件查看
│   │   ├── LogViewer.tsx         # 代理日志查看
│   │   ├── Settings.tsx          # 应用设置
│   │   └── About.tsx             # 关于页面
│   ├── hooks/                    # React Hooks
│   ├── i18n/                     # 国际化 (中文/英文)
│   ├── appearance/               # 外观与主题
│   └── presets/                  # 供应商预设
├── src-tauri/                    # 后端 (Rust + Tauri + Axum)
│   ├── src/
│   │   ├── main.rs               # Tauri 入口
│   │   ├── lib.rs                # Axum 路由与 Tauri 构建
│   │   ├── config.rs             # 配置读写、代理启停、模型映射
│   │   ├── proxy.rs              # HTTP/WebSocket 代理核心
│   │   ├── convert.rs            # 协议转换 (Responses ⇄ Chat ⇄ Anthropic)
│   │   ├── sse.rs                # SSE 流式转换
│   │   ├── models.rs             # 数据结构定义
│   │   ├── commands.rs           # Tauri IPC 命令
│   │   └── api.rs                # HTTP 管理 API
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
├── vite.config.ts
└── tsconfig.json
```

---

## 技术栈

| 层 | 技术 |
|---|---|
| 前端框架 | React 19 + TypeScript |
| UI 库 | Ant Design 6 + Tailwind CSS 4 |
| 桌面框架 | Tauri 2 |
| 后端语言 | Rust |
| HTTP 服务 | Axum 0.8 |
| SSE 转换 | 自定义流式解析器 |
| WebSocket | tokio-tungstenite |

---

## 协议

[MIT](LICENSE)

Copyright © 2026 IronLink
