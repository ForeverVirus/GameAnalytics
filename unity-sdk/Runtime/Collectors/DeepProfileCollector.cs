// GameAnalytics Device Profiler - Deep Profile Collector
// Uses Unity Profiler.logFile + enableBinaryLog to capture full call hierarchy
// including user script functions, then provides the .data file for upload.
// Also uses ProfilerRecorderHandle.GetAvailable() to dynamically discover markers.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using System;
using System.Collections.Generic;
using System.IO;
using Unity.Profiling;
using Unity.Profiling.LowLevel;
using Unity.Profiling.LowLevel.Unsafe;
using UnityEngine;
using UnityEngine.Profiling;
using GameAnalytics.Profiler.Data;

namespace GameAnalytics.Profiler.Collectors
{
    public class DeepProfileCollector
    {
        private CaptureSession _session;
        private string _currentProfilerDataBasePath;
        private string _lastCompletedDataPath;
        private string _currentSessionFileBaseName;
        private string _lastCompletedSessionFileBaseName;
        private bool _isCapturing;
        private float _captureStartTime;
        private float _durationLimit;
        private bool _previousProfilerEnabled;
        private bool _previousBinaryLogEnabled;
        private string _previousProfilerLogFile;

        // Dynamic marker discovery
        private bool _autoDiscoverMarkers;
        private int _discoveryFrameCounter;
        private const int DiscoveryInterval = 60; // re-discover every 60 frames
        private List<DynamicMarker> _dynamicMarkers = new List<DynamicMarker>();
        private HashSet<string> _knownMarkerNames = new HashSet<string>(StringComparer.Ordinal);

        // Static set of Unity internal prefixes to skip during marker discovery
        private static readonly string[] InternalPrefixes = new[]
        {
            "Unity.", "UnityEngine.", "UnityEditor.", "Profiler.",
            "GC.", "Gfx.", "GPU.", "Loading.", "Physics.",
            "Camera.", "Render.", "Culling", "Shadows.",
            "UI.", "GUI.", "Canvas.", "UGUI.",
            "Animation.", "Animator.", "Director.",
            "Particle.", "VFX.",
            "Audio.", "Video.",
            "PlayerLoop.", "PostLateUpdate.", "PreUpdate.", "PreLateUpdate.",
            "FixedUpdate.", "Update.", "EarlyUpdate.",
            "Semaphore.", "Mutex.", "Job.",
        };

        /// <summary>Full path of the .data profiler binary log file.</summary>
        public string ProfilerDataPath => _lastCompletedDataPath;

        /// <summary>File size of the profiler data file in bytes. 0 if not yet captured.</summary>
        public long ProfilerDataSize
        {
            get
            {
                try
                {
                    string actualPath = !string.IsNullOrEmpty(_lastCompletedDataPath)
                        ? _lastCompletedDataPath
                        : ResolveActualDataFilePath(_currentProfilerDataBasePath);
                    if (!string.IsNullOrEmpty(actualPath) && File.Exists(actualPath))
                        return new FileInfo(actualPath).Length;
                }
                catch { }
                return 0;
            }
        }

        /// <summary>Whether a deep capture is currently active.</summary>
        public bool IsCapturing => _isCapturing;

        /// <summary>Dynamically discovered function samples from the current frame.</summary>
        public List<FunctionSample> DynamicSamples { get; private set; } = new List<FunctionSample>();

        public void Initialize(CaptureSession session)
        {
            _session = session;
        }

        /// <summary>Start deep capture: enable Profiler binary log to file + dynamic marker discovery.</summary>
        public void OnCaptureStart(string outputDirectory, string sessionFileBaseName, float durationLimit, bool autoDiscoverMarkers)
        {
            ClearLastCaptureMetadata();
            foreach (var marker in _dynamicMarkers)
            {
                if (marker.recorder.Valid)
                    marker.recorder.Dispose();
            }
            _dynamicMarkers.Clear();
            _knownMarkerNames.Clear();
            _currentSessionFileBaseName = sessionFileBaseName;
            _durationLimit = durationLimit;
            _autoDiscoverMarkers = autoDiscoverMarkers;
            _discoveryFrameCounter = 0;

            if (!Directory.Exists(outputDirectory))
                Directory.CreateDirectory(outputDirectory);

            _currentProfilerDataBasePath = Path.Combine(outputDirectory, $"{sessionFileBaseName}.deep");

            // Enable Unity Profiler binary log to file
            _previousProfilerLogFile = UnityEngine.Profiling.Profiler.logFile;
            _previousBinaryLogEnabled = UnityEngine.Profiling.Profiler.enableBinaryLog;
            _previousProfilerEnabled = UnityEngine.Profiling.Profiler.enabled;

            UnityEngine.Profiling.Profiler.logFile = _currentProfilerDataBasePath;
            UnityEngine.Profiling.Profiler.enableBinaryLog = true;
            UnityEngine.Profiling.Profiler.enabled = true;

            _captureStartTime = Time.realtimeSinceStartup;
            _isCapturing = true;

            // Initial marker discovery
            if (_autoDiscoverMarkers)
                DiscoverMarkers();

            Debug.Log($"[GAProfiler] Deep capture started. Output: {_currentProfilerDataBasePath}");
        }

        /// <summary>Called every frame during capture.</summary>
        public void OnFrameUpdate()
        {
            if (!_isCapturing) return;

            // Check duration limit
            float elapsed = Time.realtimeSinceStartup - _captureStartTime;
            if (_durationLimit > 0 && elapsed >= _durationLimit)
            {
                Debug.Log($"[GAProfiler] Deep capture duration limit reached ({_durationLimit}s). Auto-stopping.");
                OnCaptureStop();
                return;
            }

            // Periodic marker re-discovery
            if (_autoDiscoverMarkers)
            {
                _discoveryFrameCounter++;
                if (_discoveryFrameCounter >= DiscoveryInterval)
                {
                    _discoveryFrameCounter = 0;
                    DiscoverMarkers();
                }
            }
        }

        /// <summary>Collect dynamic marker samples for the current frame.</summary>
        public List<FunctionSample> CollectDynamicSamples()
        {
            DynamicSamples.Clear();

            for (int i = 0; i < _dynamicMarkers.Count; i++)
            {
                var m = _dynamicMarkers[i];
                if (!m.recorder.Valid) continue;

                long ns = m.recorder.LastValue;
                if (ns <= 0) continue;

                float ms = ns / 1_000_000f;
                ushort nameIdx = _session.GetOrAddString(m.name);

                DynamicSamples.Add(new FunctionSample
                {
                    functionNameIndex = nameIdx,
                    category = m.category,
                    selfTimeMs = ms,
                    totalTimeMs = ms,
                    callCount = (ushort)Math.Max(1, m.recorder.Count),
                    depth = 0,
                    parentIndex = -1,
                    threadIndex = 0
                });
            }

            return DynamicSamples;
        }

        /// <summary>Stop deep capture and finalize the profiler data file.</summary>
        public void OnCaptureStop()
        {
            if (!_isCapturing) return;
            _isCapturing = false;

            // Stop binary logging
            UnityEngine.Profiling.Profiler.enableBinaryLog = _previousBinaryLogEnabled;
            UnityEngine.Profiling.Profiler.logFile = _previousProfilerLogFile ?? string.Empty;
            UnityEngine.Profiling.Profiler.enabled = _previousProfilerEnabled;

            _lastCompletedDataPath = ResolveActualDataFilePath(_currentProfilerDataBasePath);
            _lastCompletedSessionFileBaseName = _currentSessionFileBaseName;

            // Dispose dynamic marker recorders
            foreach (var m in _dynamicMarkers)
            {
                if (m.recorder.Valid)
                    m.recorder.Dispose();
            }
            _dynamicMarkers.Clear();
            _knownMarkerNames.Clear();

            // Report file info
            long fileSize = ProfilerDataSize;
            Debug.Log($"[GAProfiler] Deep capture stopped. File: {_lastCompletedDataPath} ({fileSize / (1024f * 1024f):F1} MB)");

            _currentProfilerDataBasePath = null;
            _currentSessionFileBaseName = null;
        }

        /// <summary>Get the actual profiler data file path (Unity appends .raw).</summary>
        public string GetActualDataFilePath()
        {
            return _lastCompletedDataPath;
        }

        public string GetActualDataFilePathForSession(string sessionFileBaseName, string outputDirectory)
        {
            if (string.IsNullOrEmpty(sessionFileBaseName) || string.IsNullOrEmpty(outputDirectory))
                return null;
            return ResolveActualDataFilePath(Path.Combine(outputDirectory, $"{sessionFileBaseName}.deep"));
        }

        public static string ResolveActualDataFilePath(string profilerDataBasePath)
        {
            if (string.IsNullOrEmpty(profilerDataBasePath)) return null;

            string rawPath = profilerDataBasePath + ".raw";
            if (File.Exists(rawPath)) return rawPath;
            if (File.Exists(profilerDataBasePath)) return profilerDataBasePath;
            return null;
        }

        public void ClearLastCaptureMetadata()
        {
            _lastCompletedDataPath = null;
            _lastCompletedSessionFileBaseName = null;
        }

        // ==================== Dynamic Marker Discovery ====================

        private void DiscoverMarkers()
        {
            var handles = new List<ProfilerRecorderHandle>();
            ProfilerRecorderHandle.GetAvailable(handles);
            RegisterNewMarkers(handles, FunctionCategory.Scripting, ProfilerCategory.Scripts);

            // Also check Internal category where some user markers land
            var internalHandles = new List<ProfilerRecorderHandle>();
            ProfilerRecorderHandle.GetAvailable(internalHandles);
            RegisterNewMarkers(internalHandles, FunctionCategory.Scripting, ProfilerCategory.Internal);
        }

        private void RegisterNewMarkers(List<ProfilerRecorderHandle> handles, FunctionCategory defaultCategory, ProfilerCategory allowedCategory)
        {
            foreach (var handle in handles)
            {
                var desc = ProfilerRecorderHandle.GetDescription(handle);
                if (desc.Category != allowedCategory)
                    continue;

                string name = desc.Name;

                // Skip if already tracked
                if (_knownMarkerNames.Contains(name)) continue;

                // Skip markers already sampled by the static function collector
                if (FunctionTimingCollector.IsStaticMarker(name)) continue;

                // Skip Unity internal markers
                if (IsInternalMarker(name)) continue;

                // Skip non-default markers (counters, internal flags, etc.)
                if (desc.Flags.HasFlag(MarkerFlags.Counter))
                    continue;

                _knownMarkerNames.Add(name);

                var recorder = ProfilerRecorder.StartNew(desc.Category, name, 1);
                _dynamicMarkers.Add(new DynamicMarker
                {
                    name = name,
                    category = defaultCategory,
                    recorder = recorder
                });
            }
        }

        private static bool IsInternalMarker(string name)
        {
            if (string.IsNullOrEmpty(name)) return true;

            foreach (var prefix in InternalPrefixes)
            {
                if (name.StartsWith(prefix, StringComparison.Ordinal))
                    return true;
            }

            return false;
        }

        private struct DynamicMarker
        {
            public string name;
            public FunctionCategory category;
            public ProfilerRecorder recorder;
        }
    }
}

#endif
