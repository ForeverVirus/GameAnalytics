// GameAnalytics Device Profiler - Configuration ScriptableObject

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using UnityEngine;

namespace GameAnalytics.Profiler
{
    [CreateAssetMenu(fileName = "GAProfilerConfig", menuName = "GameAnalytics/Profiler Config")]
    public class GAProfilerConfig : ScriptableObject
    {
        [Header("General")]
        [Tooltip("Target FPS for jank detection thresholds")]
        public int targetFps = 60;

        [Tooltip("Sample every N frames (1 = every frame, 2 = every other frame)")]
        [Range(1, 10)]
        public int sampleEveryNFrames = 1;

        [Header("Modules")]
        public bool enableMemory = true;
        public bool enableRendering = true;
        public bool enableModuleTiming = true;
        public bool enableJankDetection = true;
        public bool enableDeviceMetrics = true;
        public bool enableScreenshots = true;
        public bool enableOverdraw = true;

        [Header("Screenshots")]
        [Tooltip("Interval between timed screenshots in seconds")]
        public float screenshotInterval = 5f;

        [Tooltip("Thumbnail height in pixels (width scaled proportionally)")]
        public int screenshotThumbnailHeight = 480;

        [Tooltip("JPEG compression quality (0-100)")]
        [Range(1, 100)]
        public int screenshotJpegQuality = 75;

        [Tooltip("Take screenshot when FPS drops below targetFPS × this value")]
        [Range(0.1f, 0.9f)]
        public float fpsDropScreenshotThreshold = 0.6f;

        [Header("Overdraw")]
        [Tooltip("Overdraw analysis sample interval in seconds")]
        public float overdrawSampleInterval = 30f;

        [Tooltip("Overdraw visualization shader (auto-detected if null)")]
        public Shader overdrawShader;

        [Header("Network")]
        [Tooltip("Enable embedded HTTP server for WiFi data transfer")]
        public bool enableWifiTransfer = true;

        [Tooltip("HTTP server port")]
        public int httpServerPort = 9527;

        [Header("Auto-Start")]
        [Tooltip("Automatically start capturing when the app launches")]
        public bool autoStartCapture = false;

        [Tooltip("Session name for auto-start captures")]
        public string autoStartSessionName = "AutoCapture";

        [Header("Deep Profiling")]
        [Tooltip("Enable function-level deep profiling (captures per-function CPU timing for call stack analysis)")]
        public bool enableDeepProfiling = true;

        [Tooltip("Enable runtime log capture during profiling")]
        public bool captureLogs = true;

        [Tooltip("Deep profiling sample rate: capture function data every N frames (higher = lower overhead)")]
        [Range(1, 10)]
        public int deepProfilingSampleRate = 1;

        [Header("Resource Memory (v3)")]
        [Tooltip("Enable per-resource-type memory tracking (Texture, Mesh, etc.)")]
        public bool enableResourceMemory = false;

        [Tooltip("Sample resource instances every N frames (higher = lower overhead)")]
        [Range(1, 120)]
        public int resourceSampleInterval = 30;

        [Header("GPU Analysis (v3)")]
        [Tooltip("Enable GPU utilization analysis via FrameTimingManager")]
        public bool enableGPUAnalysis = false;

        [Header("Custom Modules (v3)")]
        [Tooltip("User-defined Profiler marker names to track as custom modules")]
        public string[] customMarkerNames = new string[0];
    }
}

#endif
