# Changelog

## [1.0.1] - 2026-04-02

### Fixed
- Corrected v3 `.gaprof` frame block sizing so screenshot and overdraw offsets are written correctly.
- Fixed `ResourceMemoryCollector` to explicitly call `UnityEngine.Profiling.Profiler.GetRuntimeMemorySizeLong`, avoiding the `GameAnalytics.Profiler` namespace collision.
- Exposed Deep Profiling, log capture, resource memory, GPU analysis and custom module settings in the Unity Inspector.

### Changed
- Enabled Deep Profiling by default for new configs and set the default deep profiling sample rate to every frame.
- Added clearer runtime/editor warnings when Deep Profiling is disabled, so missing function sampling is visible before export.

## [1.0.0] - 2026-04-02

### Added
- Initial release
- Frame data collection (FPS, frame time, CPU/GPU timing)
- Module timing collection (Rendering, Physics, Animation, UI, Particles, Loading, Scripts, GC)
- Memory collection (Total, Reserved, Mono, GPU, GC allocations)
- Rendering stats collection (Batches, DrawCalls, SetPass, Triangles, Vertices)
- Jank detection (normal + severe)
- Device metrics (battery level, temperature) with Android/iOS native plugins
- Screenshot capture (timed + anomaly-triggered)
- Overdraw analysis with custom shader
- Binary .gaprof export format
- Embedded HTTP server for WiFi data transfer
- Runtime UI overlay with draggable FPS display and control panel
- ScriptableObject configuration (GAProfilerConfig)
- Conditional compilation for zero overhead in release builds
