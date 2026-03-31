# GameScriptAnalytics

English | [中文](README.md)

**Local-first game project analysis tool** for Unity and Godot projects — asset dependency analysis, code structure graphs, hardcode detection, and AI-assisted analysis.

---

## Features

### 📊 Project Overview
- One-click selection of Unity / Godot project directories with automatic engine detection
- Displays asset count, script count, class count, method count and other statistics after analysis
- Analysis caching to avoid redundant scans

### 🗺️ Asset Graph
- Visualize asset dependencies (Prefab, Texture, Material, Shader, Scene, Audio, etc.)
- D3.js force-directed / radial layout with zoom, pan, and highlights
- Three-column workbench: file tree (left) + graph (center) + detail panel (right)
- Ego-centric graph expanding 1-hop neighbors on node selection
- Image thumbnail preview in the top-right corner when selecting image assets
- Upstream / downstream dependency lists with click-to-navigate

### 🔗 Code Graph
- Analyze code structure for C# (Unity) and GDScript / C# (Godot)
- Three graph levels: file, class, and method
- Visualize call relationships and inheritance hierarchies
- AI single-node analysis and deep analysis support

### 🔍 Suspected References
- Detect uncertain references from dynamic loading patterns (`Resources.Load`, Addressables, etc.)
- Confirm (promote to official reference) or ignore individual items
- Batch operations with status filtering (pending / confirmed / ignored)

### ⚠️ Hardcode Detection
- Automatically detect hardcoded values: paths, URLs, magic numbers, colors, string literals
- Grouped by file with line numbers and severity levels
- Click to locate source file

### 🤖 AI Analysis
- Integrates local AI CLI tools (Claude CLI / Codex CLI / Gemini CLI / Copilot CLI)
- Quick analysis (5-min timeout) and deep analysis (10-min timeout)
- Batch analysis: send directories to AI in batches for project-level summaries
- Results persisted per-node, auto-loaded on next session

### 🌐 Bilingual Support
- Full Chinese / English UI switching
- Exported reports support both languages

### 📤 Export
- Export human-readable analysis reports
- Export AI-readable knowledge packs to the project directory

---

## Screenshots

*(Launch the app to explore each feature page)*

---

## System Requirements

- **OS**: Windows 10+, macOS 10.15+, Linux (X11)
- **Node.js**: 18+
- **Rust**: 1.77.2+ (with Cargo)
- **AI Analysis (optional)**: At least one AI CLI tool installed
  - [Claude CLI](https://docs.anthropic.com/en/docs/claude-cli)
  - [Codex CLI](https://github.com/openai/codex)
  - [Gemini CLI](https://github.com/google-gemini/gemini-cli)
  - [GitHub Copilot CLI](https://docs.github.com/en/copilot)

---

## Installation & Build

### 1. Clone the Repository

```bash
git clone <repository-url>
cd GameAnalytics/app
```

### 2. Install Dependencies

```bash
npm install
```

### 3. Development Mode

```bash
npm run tauri dev
```

This launches the desktop window with hot-reload at `http://localhost:5173`.

### 4. Build Release

```bash
npm run tauri build
```

Build outputs:
- **Windows**: `src-tauri/target/release/gamescript-analytics.exe`
- **Installer**: `src-tauri/target/release/bundle/nsis/GameScriptAnalytics_*-setup.exe`
- **MSI**: `src-tauri/target/release/bundle/msi/GameScriptAnalytics_*.msi`

### 5. Build Debug

```bash
npm run tauri build -- --debug
```

---

## Usage

1. Launch the app and click **Select Project** on the overview page
2. Choose the root directory of a Unity or Godot project
3. Click **Start Analysis** and wait for static analysis to complete
4. Use the sidebar to navigate between analysis views:
   - **Overview**: Project statistics
   - **Asset Graph**: Browse asset dependencies
   - **Code Graph**: Browse code call structures
   - **Suspected Refs**: Review uncertain dynamic references
   - **Hardcode**: Inspect hardcoded values
   - **Settings**: Configure AI CLI and language options

### AI Analysis Setup

1. Go to the **Settings** page
2. Select an installed AI CLI tool
3. Optionally configure model name and thinking level
4. Return to a graph page, select a node, and click **AI Analyze** or **Deep Analyze**

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| Desktop Framework | [Tauri 2.0](https://v2.tauri.app/) |
| Frontend | React 19 + TypeScript + Vite 8 |
| State Management | Zustand |
| Graph Visualization | D3.js (Canvas rendering) |
| Backend | Rust (static analysis, filesystem, process management) |
| Parallel Processing | Rayon (static analysis) + Tokio (async CLI calls) |
| i18n | i18next |
| Routing | React Router 7 |

---

## Project Structure

```
GameAnalytics/
├── app/                          # Tauri app root
│   ├── src/                      # Frontend source
│   │   ├── api/tauri.ts          # Tauri command bindings
│   │   ├── components/           # Shared components (GraphCanvas, FileTree, etc.)
│   │   ├── pages/                # Page components
│   │   │   ├── Overview.tsx      # Overview
│   │   │   ├── AssetGraph.tsx    # Asset Graph
│   │   │   ├── CodeGraph.tsx     # Code Graph
│   │   │   ├── SuspectedRefs.tsx # Suspected References
│   │   │   ├── Hardcode.tsx      # Hardcode Detection
│   │   │   └── Settings.tsx      # Settings
│   │   ├── store/                # Zustand state management
│   │   └── i18n/                 # i18n configuration
│   ├── src-tauri/                # Rust backend
│   │   ├── src/
│   │   │   ├── commands.rs       # Tauri commands (21 total)
│   │   │   ├── analysis.rs       # Static analysis engine
│   │   │   └── graph/            # Graph data model & store
│   │   ├── Cargo.toml
│   │   └── tauri.conf.json
│   └── package.json
```

---

## Supported Engines

| Engine | Languages | Status |
|--------|-----------|--------|
| Unity | C# | ✅ Full support |
| Godot | GDScript, C# | ✅ Full support |

> Shader files are only analyzed as part of asset references (e.g., Material → Shader). Internal shader semantics are not parsed.

---

## License

MIT License

---

## Acknowledgements

Built with [Tauri](https://tauri.app/). Thanks to the open-source community for their contributions.
