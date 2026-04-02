# GameAnalytics Unity SDK

English | [中文](README.md)

The GameAnalytics Unity SDK collects runtime performance data from Unity Editor, Windows development builds, and mobile devices, then exports reports that can be analyzed by the GameAnalytics desktop app. It covers frame metrics, module timing, screenshots, jank, logs, resource memory, GPU-related metrics, and deep-analysis data such as function sampling and deep capture.

## Feature Overview

- Captures FPS, CPU, GPU, memory, and rendering stats
- Captures module timelines for rendering, scripting, UI, loading, physics, animation, particles, GPU sync, and more
- Captures jank frames, temperature, battery, and runtime logs
- Captures screenshots and overdraw samples
- Exports `.gaprof` reports for desktop analysis
- Supports function-level sampling for module ranking, selected-frame function details, and CPU call stacks
- Supports deep capture for fuller deep-analysis data in the desktop workflow
- Includes an embedded WiFi HTTP server so the desktop app can control capture and download reports remotely

## Advantages

- Built for real devices, not only Editor-side inspection
- Low integration cost through a Unity package workflow
- Supports both lightweight reports and deeper diagnostic captures
- Works with the desktop app for a unified live-capture and offline-analysis flow
- Includes an in-game overlay for fast validation during development

## Layout

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

## Environment

- Unity 2020.3 or later
- `Development Build` or `UNITY_EDITOR`
- Recommended for profiling, verification, and performance investigations during development

## Installation

### Option 1: Local Package via Unity Package Manager

In Unity:

`Window > Package Manager > Add package from disk...`

Select:

```text
unity-sdk/package.json
```

### Option 2: Git Package

If the repository is reachable by Unity Package Manager, you can also add `com.gameanalytics.profiler` as a Git package.

## Quick Start

### 1. Create a Config Asset

From the Unity top menu:

`GameAnalytics > Profiler > Create Config Asset`

This creates a `GAProfilerConfig` asset.

### 2. Add the Profiler to the Scene

From the Unity top menu:

`GameAnalytics > Profiler > Add Profiler to Scene`

This adds:

- `GAProfiler`
- `ProfilerOverlay`

### 3. Assign the Config

Assign the created `GAProfilerConfig` asset to `GAProfiler.config` in the scene.

### 4. Apply Recommended Defaults

In the `GAProfilerConfig` Inspector, click:

`Apply Recommended Analysis Defaults`

Recommended minimum settings:

- `enableDeepProfiling`
- `captureLogs`
- `enableWifiTransfer`

For fuller deep analysis, also enable:

- `enableDeepCapture`

## Capture Modes

### Mode 1: Runtime Overlay

When the app starts, an FPS floating button appears on screen. Expanding it allows you to:

- enter a session name
- toggle deep capture
- start capture
- stop and export
- check current WiFi address and port

Default port:

- `9527`

### Mode 2: Remote Control from Desktop

After connecting from the desktop app through device IP + port, you can:

- start capture remotely
- stop capture remotely
- download `.gaprof`
- download deep data
- open and analyze the report automatically on desktop

## Report Types

### Base Report

The base report is exported as `.gaprof` and includes:

- frame timing
- FPS
- CPU / GPU timing
- memory
- rendering stats
- module timelines
- jank analysis
- runtime logs
- screenshots and overdraw

### Deep Profiling

When `enableDeepProfiling` is enabled, the desktop app can use function samples to show:

- module function ranking
- selected-frame function details
- CPU call stacks
- jank frame function details

### Deep Capture

When `enableDeepCapture` is enabled, the SDK also writes a deep data file for more complete call-hierarchy analysis. This mode has higher runtime overhead and larger output files, so it is best used for targeted investigations rather than always-on capture.

## Common Settings

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

## Recommended Workflows

### Regular Performance Validation

1. Apply recommended defaults
2. Enable `enableDeepProfiling`
3. Run and capture in Editor or on device
4. Open the resulting `.gaprof` in the desktop app
5. Review modules, jank, logs, call stacks, and AI analysis

### Targeted Deep Investigation

1. Enable `enableDeepCapture` while idle
2. Keep the capture window controlled so deep files do not grow too large
3. Let the desktop app download deep data after capture stops
4. Review the merged deep report on desktop

## Output Location

Captured files are stored by default at:

```text
Application.persistentDataPath/GameAnalytics
```

You can open the folder from the Unity menu:

`GameAnalytics > Profiler > Open Data Folder`

## Working with the Desktop App

The desktop app reads SDK-generated report files and provides:

- report history management
- preload of report pages
- CPU call stacks and module function tables
- jank and screenshot navigation
- AI performance assistant

## License

MIT
