# GameAnalytics

[English](README_EN.md) | 中文

GameAnalytics 是一套面向 Unity / Godot 项目的本地优先分析工具，覆盖静态工程分析、真机性能采集、离线性能报告、深度函数采样和 AI 辅助诊断。它的目标不是只给你一堆指标，而是把“项目结构”和“运行时问题”放到同一套工作流里，帮助团队更快定位性能瓶颈、资源问题和代码风险。

桌面端基于 [Tauri 2](https://v2.tauri.app/) 构建，结合 Rust 后端与 Web 前端工作台，兼顾本地性能、原生打包和跨模块桌面交互能力。

## 核心能力

### 静态工程分析
- 自动识别 Unity / Godot 项目并建立项目索引
- 统计资源、脚本、类、方法等项目规模信息
- 构建资源依赖图谱，查看上游 / 下游引用关系
- 构建代码图谱，支持文件级、类级、方法级分析
- 检测疑似动态引用，辅助梳理 `Resources.Load`、Addressables 等场景
- 检测硬编码值，如路径、URL、魔数、颜色和字符串字面量

### 运行时性能分析
- 连接 Unity Editor 或真机设备，实时采集 FPS、CPU、GPU、内存、渲染指标
- 支持停止采集后自动生成 `.gaprof` 报告
- 支持历史报告管理、导入、再次打开和项目内复用
- 支持截图、卡顿帧、运行日志、模块时间线、资源内存分析
- 支持深度采样报告接入，查看 CPU 调用堆栈、模块函数排行、卡顿函数详情

### AI 辅助分析
- 集成本地 AI CLI 工具，支持 Claude CLI、Codex CLI、Gemini CLI、Copilot CLI
- 可对单个图谱节点、整份性能报告或单个模块做 AI 分析
- 支持对话式性能分析助手，结合报告内容持续追问
- AI 日志和结果可在桌面端持续查看

## 产品优势

- 本地优先：分析、报告和 AI 调用链路都围绕本地工程工作区设计，适合企业内网和项目私有数据场景
- 一体化：静态图谱、真机采集、离线报告、深度分析、AI 诊断在一个桌面工具里完成
- 对真机友好：既支持连线实时采集，也支持采集结束后离线分析报告
- 面向定位问题：不只显示概览分数，还能追到模块、帧、函数和调用树
- 适合迭代验证：历史报告、再次打开、项目内缓存和报告复用都已经打通

## 功能模块

### 代码与资源工作台
- 总览
- 资源图谱
- 代码图谱
- 疑似引用池
- 硬编码检测

### 性能报告工作台
- 性能简报
- 运行信息
- 模块耗时统计
- CPU 调用堆栈
- 渲染 / GPU 同步 / 脚本 / UI / 加载 / 物理 / 动画 / 粒子 / GPU / 自定义模块
- 卡顿分析
- 内存分析
- 电量与温度
- 运行日志
- 截图浏览
- 历史报告
- AI 性能分析助手

## 适用场景

- 客户端性能优化与回归对比
- 真机问题复现后的离线报告排查
- 项目资源依赖梳理与冗余治理
- 代码结构摸底和风险点排查
- 技术美术、客户端、TA、性能优化工程师协作

## 系统要求

- Windows 10 或更高版本
- Node.js 18+
- Rust 1.77.2+（含 Cargo）
- 可选：至少安装一个本地 AI CLI

当前仓库的桌面端构建、调试和打包流程以 Windows 为正式支持平台。

## 安装与编译

### 1. 克隆仓库

```bash
git clone <repository-url>
cd GameAnalytics/app
```

### 2. 安装依赖

```bash
npm install
```

### 3. 开发模式启动

```bash
npm run tauri dev
```

### 4. 编译 Debug 版本

```bash
npm run tauri build -- --debug
```

输出路径：

- `app/src-tauri/target/debug/gamescript-analytics.exe`

### 5. 编译 Release 版本

```bash
npm run tauri build
```

输出路径：

- `app/src-tauri/target/release/gamescript-analytics.exe`
- `app/src-tauri/target/release/bundle/nsis/GameScriptAnalytics_*-setup.exe`
- `app/src-tauri/target/release/bundle/msi/GameScriptAnalytics_*.msi`

## 使用方式

### 静态工程分析

1. 启动桌面端
2. 选择 Unity 或 Godot 项目目录
3. 点击开始分析
4. 在左侧页面切换到资源图谱、代码图谱、疑似引用和硬编码页面查看结果

### 性能报告分析

1. 打开同一个项目
2. 进入性能分析页面
3. 选择以下任一方式生成或查看报告

- 连接 Unity Editor / 真机设备后实时采集
- 从已有 `.gaprof` 报告导入
- 从项目历史报告中直接打开

4. 报告加载完成后，在各分页中查看模块、调用堆栈、卡顿、内存、日志和截图

### AI 使用

1. 进入设置页配置 AI CLI
2. 可选填写模型名与思考强度
3. 在图谱页或性能报告页直接触发 AI 分析

## Unity SDK

仓库内置 Unity 侧采集 SDK，位于：

- [unity-sdk/README.md](unity-sdk/README.md)
- [unity-sdk/README_EN.md](unity-sdk/README_EN.md)

它负责在 Unity Editor 或真机端采集运行时数据，并把结果导出为桌面端可分析的报告文件。

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | [Tauri 2](https://v2.tauri.app/) |
| 前端 | React 19 + TypeScript + Vite |
| 后端 | Rust |
| 状态管理 | Zustand |
| 图形可视化 | D3.js |
| 国际化 | i18next |
| Unity SDK | C# + Unity Runtime / Editor API |

## 仓库结构

```text
GameAnalytics/
├── app/                 # 桌面端（Tauri + React + Rust）
├── unity-sdk/           # Unity 运行时采集 SDK
├── README.md
└── README_EN.md
```

## 社区

学 AI，上 L 站 — [LinuxDO](https://linux.do/)
