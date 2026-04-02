# GameAnalytics Unity SDK

[English](README_EN.md) | 中文

GameAnalytics Unity SDK 用于在 Unity Editor、Windows 开发包或移动设备上采集运行时性能数据，并把结果导出为桌面端可读取的性能报告。它覆盖基础帧指标、模块耗时、截图、卡顿、日志、资源内存、GPU 相关指标，以及面向深度分析的函数采样与 deep capture。

## 功能概览

- 采集 FPS、CPU、GPU、内存、渲染统计
- 采集模块级时间线：渲染、脚本、UI、加载、物理、动画、粒子、GPU 同步等
- 采集卡顿帧、设备温度、电量、运行日志
- 采集截图与 overdraw 采样
- 输出 `.gaprof` 报告供桌面端分析
- 支持函数级采样，用于模块函数排行、选中帧函数详情、CPU 调用堆栈
- 支持 deep capture，输出更完整的深度采样数据并接入桌面端 deep 分析流程
- 内置 WiFi HTTP 服务，支持桌面端远程控制采集与下载报告

## 优势

- 面向真机：不是只在 Editor 里看数据，可以直接采集设备运行证据
- 低接入成本：作为 Unity 包导入后即可配置
- 同时支持轻量报告与深度报告
- 可与桌面端联动，形成实时采集、停止导出、离线复盘的一体化流程
- 支持运行时悬浮面板，便于现场调试

## 目录结构

```text
unity-sdk/
├── Runtime/
│   ├── GAProfiler.cs
│   ├── GAProfilerConfig.cs
│   ├── Collectors/
│   ├── Data/
│   ├── Network/
│   └── UI/
├── Editor/
│   ├── GAProfilerEditor.cs
│   └── DeepProfileExporter.cs
└── package.json
```

## 适用环境

- Unity 2020.3 及以上
- `Development Build` / `UNITY_EDITOR`
- 推荐在性能验证与开发调试阶段使用

## 安装方式

### 方式一：Unity Package Manager 本地包

在 Unity 中打开：

`Window > Package Manager > Add package from disk...`

选择：

```text
unity-sdk/package.json
```

### 方式二：通过 Git 包接入

如果仓库可被 Unity Package Manager 访问，也可以直接以 Git URL 的方式添加 `com.gameanalytics.profiler`。

当前仓库的 Unity SDK 位于主仓库子目录，因此可以直接使用：

```text
https://github.com/ForeverVirus/GameAnalytics.git?path=/unity-sdk
```

如果要锁定某个 tag / 分支 / commit，可以在 URL 末尾追加版本锚点，例如：

```text
https://github.com/ForeverVirus/GameAnalytics.git?path=/unity-sdk#master
```

或：

```text
https://github.com/ForeverVirus/GameAnalytics.git?path=/unity-sdk#v1.0.0
```

注意：

- 如果仓库是私有的，安装者必须有该仓库的访问权限
- `path=/unity-sdk` 不能省略，否则 Unity 会把整个仓库根目录当成包来解析

## 快速接入

### 1. 创建配置资产

在 Unity 顶部菜单中执行：

`GameAnalytics > Profiler > Create Config Asset`

会生成一个 `GAProfilerConfig` 资产。

### 2. 把采集器加入场景

在 Unity 顶部菜单中执行：

`GameAnalytics > Profiler > Add Profiler to Scene`

这一步会自动创建：

- `GAProfiler`
- `ProfilerOverlay`

### 3. 绑定配置

把刚创建的 `GAProfilerConfig` 指定到场景中的 `GAProfiler.config`。

### 4. 配置推荐项

在 `GAProfilerConfig` Inspector 里点击：

`Apply Recommended Analysis Defaults`

推荐至少开启：

- `enableDeepProfiling`
- `captureLogs`
- `enableWifiTransfer`

如需更完整的深度报告，可再开启：

- `enableDeepCapture`

## 采集方式

### 方式一：运行时悬浮面板

运行后，屏幕上会出现 FPS 悬浮按钮。展开后可：

- 输入会话名
- 开关深度采集
- 开始采集
- 停止并导出
- 查看当前 WiFi 地址和端口

默认端口：

- `9527`

### 方式二：桌面端远程控制

打开桌面端后，可以通过设备 IP + 端口连接 SDK：

- 远程开始采集
- 远程停止采集
- 下载 `.gaprof`
- 下载 deep 数据
- 自动在桌面端打开并分析报告

## 报告类型

### 基础报告

基础报告会导出为 `.gaprof`，包含：

- 帧时间
- FPS
- CPU / GPU 时间
- 内存
- 渲染统计
- 模块时间线
- 卡顿分析
- 运行日志
- 截图与 overdraw

### 深度采样

开启 `enableDeepProfiling` 后，桌面端可以基于函数采样展示：

- 模块函数排行
- 选中帧函数详情
- CPU 调用堆栈
- 卡顿帧函数信息

### Deep Capture

开启 `enableDeepCapture` 后，SDK 会额外写出 deep 数据文件，用于更完整的深度调用分析。这个模式开销更高、文件更大，适合专项排查，不建议默认常开。

## 常用配置项

### General

- `targetFps`
- `sampleEveryNFrames`

### Modules

- `enableMemory`
- `enableRendering`
- `enableModuleTiming`
- `enableJankDetection`
- `enableDeviceMetrics`
- `enableScreenshots`
- `enableOverdraw`

### Network

- `enableWifiTransfer`
- `httpServerPort`

### Deep Profiling

- `enableDeepProfiling`
- `captureLogs`
- `deepProfilingSampleRate`

### Deep Capture

- `enableDeepCapture`
- `deepCaptureDurationLimit`
- `autoDiscoverMarkers`

### Advanced

- `enableResourceMemory`
- `resourceSampleInterval`
- `enableGPUAnalysis`
- `customMarkerNames`

## 推荐工作流

### 常规性能回归

1. 使用推荐默认配置
2. 开启 `enableDeepProfiling`
3. 在设备或 Editor 中运行并采集
4. 停止后在桌面端打开 `.gaprof`
5. 查看模块、卡顿、日志、调用堆栈和 AI 分析结果

### 专项深度排查

1. 在空闲状态下开启 `enableDeepCapture`
2. 控制采集时长，避免 deep 文件过大
3. 停止采集后让桌面端下载 deep 数据
4. 在桌面端查看 deep 合并后的报告

## 输出位置

SDK 导出的采集文件默认保存在：

```text
Application.persistentDataPath/GameAnalytics
```

可通过 Unity 菜单直接打开：

`GameAnalytics > Profiler > Open Data Folder`

## 与桌面端配合

桌面端会读取 SDK 生成的报告文件，并提供：

- 报告历史管理
- 预加载各分页数据
- CPU 调用堆栈与模块函数表
- 卡顿与截图定位
- AI 性能分析助手
