# Changelog

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
