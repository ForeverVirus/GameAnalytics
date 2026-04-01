# GameScriptAnalytics

[English](README_EN.md) | 中文

**本地优先的游戏项目分析工具**，支持 Unity 和 Godot 项目的资源依赖分析、代码结构图谱、硬编码检测和 AI 辅助分析。

---

## 功能特性

### 📊 项目总览
- 一键选择 Unity / Godot 项目目录，自动识别引擎类型
- 静态分析完成后展示资源数、脚本数、类数、方法数等统计数据
- 支持分析缓存，避免重复扫描

### 🗺️ 资源图谱（Asset Graph）
- 可视化展示资源之间的依赖关系（Prefab、Texture、Material、Shader、Scene、Audio 等）
- D3.js 力导向 / 径向布局，支持缩放、拖拽、高亮
- 左侧文件树 + 中间图谱 + 右侧详情的三栏工作台
- 选中节点后以自我中心图展开一跳邻居
- 图片类资源选中后右上角显示缩略图预览
- 上游/下游依赖列表，点击可跳转

### 🔗 代码图谱（Code Graph）
- 分析 C#（Unity）和 GDScript / C#（Godot）的代码结构
- 支持文件级、类级、方法级三层图谱切换
- 代码节点之间的调用关系、继承关系可视化
- 同样支持 AI 单节点分析和深度分析

### 🔍 疑似引用池（Suspected References）
- 检测 `Resources.Load`、Addressables 等动态加载模式的不确定引用
- 支持逐条确认（提升为正式引用）或忽略
- 批量操作，按状态筛选（待审核 / 已确认 / 已忽略）

### ⚠️ 硬编码检测（Hardcode Detection）
- 自动检测代码中的硬编码值：路径、URL、魔术数字、颜色值、字符串字面量
- 按文件分组展示，标注行号和严重级别
- 点击可定位到源文件

### 🤖 AI 分析
- 集成本地 AI CLI 工具（Claude CLI / Codex CLI / Gemini CLI / Copilot CLI，当前按 Windows 环境适配）
- 快速分析（5 分钟超时）和深度分析（10 分钟超时）
- 批量分析：按目录逐批发送给 AI，生成项目级摘要
- 分析结果持久化到每个节点，下次打开自动加载

### 🌐 双语支持
- 完整的中文 / 英文界面切换
- 导出报告同样支持双语

### 📤 导出
- 导出人类可读的分析报告
- 导出 AI 可读的知识包到项目目录

---

## 截图

*(启动应用后可查看各功能页面)*

---

## 系统要求

- **操作系统**：Windows 10+（当前正式支持平台）
- **Node.js**：18+
- **Rust**：1.77.2+（含 Cargo）
- **AI 分析（可选）**：需安装至少一个 AI CLI 工具
  - [Claude CLI](https://docs.anthropic.com/en/docs/claude-cli)
  - [Codex CLI](https://github.com/openai/codex)
  - [Gemini CLI](https://github.com/google-gemini/gemini-cli)
  - [GitHub Copilot CLI](https://docs.github.com/en/copilot)

### 平台说明

- 当前版本只把 Windows 作为正式支持和验证平台。
- 仓库里虽然保留了少量 macOS / Linux 分支代码（例如文件管理器打开逻辑、Tauri 图标资源），但没有完成这些平台上的 AI CLI 启动、PATH 继承、打包与回归验证。
- 因此目前不要把 macOS / Linux 视为已支持平台；如果后续补完验证流程，再单独恢复文档声明。

---

## 安装与编译

### 1. 克隆项目

```bash
git clone <仓库地址>
cd GameAnalytics/app
```

### 2. 安装依赖

```bash
npm install
```

### 3. 开发模式

```bash
npm run tauri dev
```

启动后会自动打开桌面窗口，前端热更新地址为 `http://localhost:5173`。

### 4. 编译 Release 版本

```bash
npm run tauri build
```

编译产物位于：
- **Windows**: `src-tauri/target/release/gamescript-analytics.exe`
- **安装包**: `src-tauri/target/release/bundle/nsis/GameScriptAnalytics_*-setup.exe`
- **MSI**: `src-tauri/target/release/bundle/msi/GameScriptAnalytics_*.msi`

当前文档只维护 Windows 构建产物说明。

### 5. 编译 Debug 版本

```bash
npm run tauri build -- --debug
```

---

## 使用方法

1. 启动应用，在总览页面点击 **选择项目** 按钮
2. 选择 Unity 或 Godot 项目的根目录
3. 点击 **开始分析**，等待静态分析完成
4. 通过左侧导航栏切换不同分析维度：
   - **总览**：查看项目统计信息
   - **资源图谱**：浏览资源依赖关系
   - **代码图谱**：浏览代码调用结构
   - **疑似引用**：审核不确定的动态引用
   - **硬编码**：检查硬编码值
   - **设置**：配置 AI CLI 和语言选项

### AI 分析配置

1. 进入 **设置** 页面
2. 选择已安装的 AI CLI 工具
3. 可选配置模型名称和思考深度
4. 返回图谱页面，选中节点后点击 **AI 分析** 或 **深度分析**

---

## 技术栈

| 层级 | 技术 |
|------|------|
| 桌面框架 | [Tauri 2.0](https://v2.tauri.app/) |
| 前端 | React 19 + TypeScript + Vite 8 |
| 状态管理 | Zustand |
| 图谱可视化 | D3.js（Canvas 渲染） |
| 后端 | Rust（静态分析、文件系统、进程管理） |
| 并行处理 | Rayon（静态分析）+ Tokio（异步 CLI 调用） |
| 国际化 | i18next |
| 路由 | React Router 7 |

---

## 项目结构

```
GameAnalytics/
├── app/                          # Tauri 应用主目录
│   ├── src/                      # 前端源码
│   │   ├── api/tauri.ts          # Tauri 命令绑定
│   │   ├── components/           # 通用组件（GraphCanvas, FileTree 等）
│   │   ├── pages/                # 页面组件
│   │   │   ├── Overview.tsx      # 总览
│   │   │   ├── AssetGraph.tsx    # 资源图谱
│   │   │   ├── CodeGraph.tsx     # 代码图谱
│   │   │   ├── SuspectedRefs.tsx # 疑似引用
│   │   │   ├── Hardcode.tsx      # 硬编码检测
│   │   │   └── Settings.tsx      # 设置
│   │   ├── store/                # Zustand 状态管理
│   │   └── i18n/                 # 国际化配置
│   ├── src-tauri/                # Rust 后端
│   │   ├── src/
│   │   │   ├── commands.rs       # Tauri 命令（21 个）
│   │   │   ├── analysis.rs       # 静态分析引擎
│   │   │   └── graph/            # 图数据模型与存储
│   │   ├── Cargo.toml
│   │   └── tauri.conf.json
│   └── package.json
```

---

## 支持的引擎

| 引擎 | 语言 | 状态 |
|------|------|------|
| Unity | C# | ✅ 完整支持 |
| Godot | GDScript, C# | ✅ 完整支持 |

> Shader 文件仅作为资源引用的一部分参与分析（如 Material → Shader），不解析 Shader 内部语义。

---

## 许可证

MIT License

---

## 致谢

本项目使用 [Tauri](https://tauri.app/) 构建，感谢所有开源社区的贡献。
