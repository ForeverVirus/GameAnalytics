# GameScriptAnalytics

English | [中文](README.md)

GameScriptAnalytics is a local-first analysis toolkit for Unity and Godot projects. It combines static project inspection, device-side performance capture, offline report analysis, deep function sampling, and AI-assisted diagnosis in one desktop workflow. The goal is not just to show metrics, but to connect project structure and runtime evidence so teams can locate bottlenecks faster.

The desktop application is built on [Tauri 2](https://v2.tauri.app/), combining a Rust backend with a web-based workspace to deliver local performance, native packaging, and practical desktop integration.

## Core Capabilities

### Static Project Analysis
- Detects Unity and Godot projects automatically
- Indexes assets, scripts, classes, methods, and project-scale metadata
- Builds asset dependency graphs for upstream and downstream tracing
- Builds code graphs at file, class, and method levels
- Detects suspected dynamic references such as `Resources.Load` and Addressables usage
- Detects hardcoded values such as paths, URLs, magic numbers, colors, and string literals

### Runtime Performance Analysis
- Connects to Unity Editor or physical devices for live capture
- Generates `.gaprof` reports after capture stops
- Supports importing, reopening, and managing report history per project
- Includes screenshots, jank analysis, runtime logs, module timelines, and resource memory analysis
- Supports deep sampled reports for CPU call trees, per-module function ranking, and jank frame function details

### AI-Assisted Analysis
- Integrates local AI CLI tools including Claude CLI, Codex CLI, Gemini CLI, and Copilot CLI
- Can analyze graph nodes, full performance reports, or individual modules
- Includes a conversational performance assistant on top of report data
- Streams AI logs and keeps analysis results available in the desktop UI

## Product Advantages

- Local-first workflow suitable for private projects and internal environments
- Unified desktop experience for static graphs, device capture, offline reports, deep analysis, and AI diagnosis
- Device-friendly workflow with both live capture and offline report investigation
- Built for root-cause analysis, not only high-level scores: modules, frames, functions, and call trees are all traceable
- Practical for iteration: report history, reopen flow, per-project reuse, and cached analysis are already integrated

## Feature Areas

### Code and Asset Workspace
- Overview
- Asset Graph
- Code Graph
- Suspected References
- Hardcode Detection

### Performance Report Workspace
- Performance Summary
- Runtime Info
- Module Timing Overview
- CPU Call Stacks
- Rendering / GPU Sync / Scripting / UI / Loading / Physics / Animation / Particles / GPU / Custom Modules
- Jank Analysis
- Memory Analysis
- Battery and Temperature
- Runtime Logs
- Screenshot Viewer
- Report History
- AI Performance Assistant

## Typical Use Cases

- Performance optimization and regression validation
- Offline investigation after reproducing issues on device
- Asset dependency cleanup and redundancy review
- Code structure inspection and risk discovery
- Collaboration across client engineers, TA, technical artists, and performance engineers

## System Requirements

- Windows 10 or later
- Node.js 18+
- Rust 1.77.2+ with Cargo
- Optional: at least one local AI CLI

The desktop build, debug, and packaging flow in this repository is currently validated on Windows.

## Installation and Build

### 1. Clone the Repository

```bash
git clone <repository-url>
cd GameAnalytics/app
```

### 2. Install Dependencies

```bash
npm install
```

### 3. Run in Development Mode

```bash
npm run tauri dev
```

### 4. Build Debug

```bash
npm run tauri build -- --debug
```

Output:

- `app/src-tauri/target/debug/gamescript-analytics.exe`

### 5. Build Release

```bash
npm run tauri build
```

Output:

- `app/src-tauri/target/release/gamescript-analytics.exe`
- `app/src-tauri/target/release/bundle/nsis/GameScriptAnalytics_*-setup.exe`
- `app/src-tauri/target/release/bundle/msi/GameScriptAnalytics_*.msi`

## How to Use

### Static Project Analysis

1. Launch the desktop app
2. Select a Unity or Godot project directory
3. Start analysis
4. Navigate through asset graphs, code graphs, suspected references, and hardcode results

### Performance Report Analysis

1. Open the same project in the desktop app
2. Go to the performance analysis workspace
3. Generate or open a report through one of these paths

- Connect to Unity Editor or a physical device and capture live
- Import an existing `.gaprof` report
- Open a saved report from project history

4. Review modules, call stacks, jank, memory, logs, and screenshots after the report finishes loading

### AI Setup

1. Configure an AI CLI in Settings
2. Optionally set a model name and thinking level
3. Trigger AI analysis from graph pages or report pages

## Unity SDK

The repository also includes the Unity-side capture SDK:

- `unity-sdk/README.md`
- `unity-sdk/README_EN.md`

It is responsible for collecting runtime data in Unity Editor or on device and exporting reports for the desktop app.

## Tech Stack

| Layer | Technology |
|------|------|
| Desktop Framework | [Tauri 2](https://v2.tauri.app/) |
| Frontend | React 19 + TypeScript + Vite |
| Backend | Rust |
| State Management | Zustand |
| Visualization | D3.js |
| Internationalization | i18next |
| Unity SDK | C# + Unity Runtime / Editor APIs |

## Repository Layout

```text
GameAnalytics/
├── app/                 # Desktop app (Tauri + React + Rust)
├── unity-sdk/           # Unity runtime capture SDK
├── README.md
└── README_EN.md
```
