// GameAnalytics Device Profiler - Data Structures
// All frame-level data captured by collectors is stored here.
// Conditionally compiled: only active in DEVELOPMENT_BUILD or UNITY_EDITOR.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using System;
using System.Collections.Generic;

namespace GameAnalytics.Profiler.Data
{
    /// <summary>
    /// Category for function-level profiling, matching UWA's classification.
    /// </summary>
    public enum FunctionCategory : byte
    {
        Rendering = 0,
        Scripting = 1,
        Physics = 2,
        Animation = 3,
        UI = 4,
        Loading = 5,
        Particles = 6,
        Sync = 7,        // GPU sync / present wait
        Overhead = 8,
        GC = 9,
        Other = 10
    }

    /// <summary>
    /// A single function timing sample within a frame.
    /// Used for deep profiling to capture per-function CPU call stacks.
    /// </summary>
    [Serializable]
    public struct FunctionSample
    {
        public ushort functionNameIndex;  // index into string table
        public FunctionCategory category;
        public float selfTimeMs;
        public float totalTimeMs;
        public ushort callCount;
        public byte depth;               // call stack depth (0 = root)
        public short parentIndex;        // -1 = root, else index into this frame's sample list
    }

    /// <summary>
    /// A runtime log entry captured during profiling session.
    /// </summary>
    [Serializable]
    public class LogEntry
    {
        public float timestamp;
        public string message;
        public string stackTrace;
        public UnityEngine.LogType logType;
        public int frameIndex;
    }

    /// <summary>
    /// Complete data for a single captured frame.
    /// Fixed-layout struct for efficient binary serialization (~200 bytes per frame).
    /// </summary>
    [Serializable]
    public struct FrameData
    {
        // Timing
        public float timestamp;       // seconds since capture start
        public float deltaTime;       // Time.unscaledDeltaTime
        public float fps;             // 1 / deltaTime
        public float cpuTimeMs;       // CPU main thread time (FrameTimingManager)
        public float gpuTimeMs;       // GPU time (FrameTimingManager)

        // Module timings (ms) — indexed by ModuleIndex enum
        public float renderTime;
        public float scriptsUpdateTime;
        public float scriptsLateUpdateTime;
        public float physicsTime;
        public float animationTime;
        public float uiTime;
        public float particleTime;
        public float loadingTime;
        public float gcCollectTime;
        public float fixedUpdateTime;
        public float renderSubmitTime;
        public float otherTime;

        // Memory (bytes)
        public long totalAllocated;
        public long totalReserved;
        public long monoHeapSize;
        public long monoUsedSize;
        public long gfxMemory;
        public long gcAllocBytes;     // GC allocation this frame

        // Rendering
        public int batches;
        public int drawCalls;
        public int setPassCalls;
        public int triangles;
        public int vertices;
        public int shadowCasters;
        public int visibleSkinnedMeshes;

        // Jank
        public byte jankLevel;        // 0=normal, 1=jank, 2=severe jank

        // Hardware
        public float batteryLevel;    // 0-1
        public float temperature;     // Celsius

        // Scene (index into string table)
        public ushort sceneIndex;
    }

    /// <summary>
    /// One-time device information snapshot.
    /// </summary>
    [Serializable]
    public class DeviceInfo
    {
        public string deviceModel;
        public string operatingSystem;
        public string processorType;
        public int processorCount;
        public int processorFrequency;
        public int systemMemoryMB;
        public int graphicsMemoryMB;
        public string graphicsDeviceName;
        public string graphicsDeviceType;
        public int screenWidth;
        public int screenHeight;
        public int screenRefreshRate;
        public int qualityLevel;
        public string qualityName;
        public string unityVersion;
        public string sdkVersion;
        public string projectName;
        public string buildGuid;
    }

    /// <summary>
    /// A screenshot captured at a specific frame.
    /// </summary>
    [Serializable]
    public class ScreenshotEntry
    {
        public int frameIndex;
        public float timestamp;
        public byte[] jpegData;
        public ScreenshotTrigger trigger;
    }

    public enum ScreenshotTrigger : byte
    {
        Timed = 0,
        FpsDrop = 1,
        MemorySpike = 2,
        Jank = 3,
        Manual = 4
    }

    /// <summary>
    /// A single overdraw measurement sample.
    /// </summary>
    [Serializable]
    public class OverdrawSample
    {
        public int frameIndex;
        public float timestamp;
        public float avgOverdrawLayers;
        public byte[] heatmapJpeg;
    }

    /// <summary>
    /// Indexes for module timing array. Matches FrameData field order.
    /// </summary>
    public enum ModuleIndex
    {
        Render = 0,
        ScriptsUpdate = 1,
        ScriptsLateUpdate = 2,
        Physics = 3,
        Animation = 4,
        UI = 5,
        Particle = 6,
        Loading = 7,
        GCCollect = 8,
        FixedUpdate = 9,
        RenderSubmit = 10,
        Other = 11
    }

    /// <summary>
    /// Complete capture session data ready for serialization.
    /// </summary>
    [Serializable]
    public class CaptureSession
    {
        public DeviceInfo deviceInfo;
        public List<FrameData> frames = new List<FrameData>();
        public List<ScreenshotEntry> screenshots = new List<ScreenshotEntry>();
        public List<OverdrawSample> overdrawSamples = new List<OverdrawSample>();
        public List<string> stringTable = new List<string>();
        public float duration;
        public string sessionName;
        public DateTime startTime;

        // Deep profiling: per-frame function samples
        public List<List<FunctionSample>> frameFunctionSamples = new List<List<FunctionSample>>();

        // Runtime logs
        public List<LogEntry> logEntries = new List<LogEntry>();

        // Config snapshot
        public bool deepProfilingEnabled;
        public int targetFps = 60;

        public ushort GetOrAddString(string s)
        {
            if (string.IsNullOrEmpty(s)) s = "<unknown>";
            int idx = stringTable.IndexOf(s);
            if (idx >= 0) return (ushort)idx;
            stringTable.Add(s);
            return (ushort)(stringTable.Count - 1);
        }
    }
}

#endif
