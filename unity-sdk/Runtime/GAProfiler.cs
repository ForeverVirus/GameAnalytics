// GameAnalytics Device Profiler - Main Manager
// Singleton MonoBehaviour that orchestrates all data collection.
// Attach to a GameObject or let it auto-create via GAProfilerConfig.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using System;
using System.Collections;
using System.Collections.Generic;
using UnityEngine;
using UnityEngine.SceneManagement;
using GameAnalytics.Profiler.Collectors;
using GameAnalytics.Profiler.Data;

namespace GameAnalytics.Profiler
{
    public enum CaptureState
    {
        Idle,
        Capturing,
        Exporting
    }

    public class GAProfiler : MonoBehaviour
    {
        public static GAProfiler Instance { get; private set; }

        [Header("Configuration")]
        public GAProfilerConfig config;

        // State
        public CaptureState State { get; private set; } = CaptureState.Idle;
        public int CapturedFrameCount => _session?.frames.Count ?? 0;
        public float CaptureElapsed => _captureStartTime > 0 ? Time.realtimeSinceStartup - _captureStartTime : 0f;
        public float CurrentFps { get; private set; }

        // Events
        public event Action OnCaptureStarted;
        public event Action<SessionExportInfo> OnCaptureStopped;
        public event Action<float> OnExportProgress;

        // Collectors
        private FrameDataCollector _frameCollector;
        private MemoryCollector _memoryCollector;
        private RenderingCollector _renderingCollector;
        private ModuleTimingCollector _moduleTimingCollector;
        private JankDetector _jankDetector;
        private DeviceMetricsCollector _deviceMetricsCollector;
        private ScreenshotCollector _screenshotCollector;
        private OverdrawCollector _overdrawCollector;

        // Deep profiling collectors
        private FunctionTimingCollector _functionTimingCollector;
        private LogCollector _logCollector;

        private List<ICollector> _activeCollectors = new List<ICollector>();

        // Session data
        private CaptureSession _session;
        private float _captureStartTime;
        private int _framesSinceCapture;
        private int _sampleEveryNFrames = 1;

        // Serializer
        private ProfileDataWriter _dataWriter;

        // Network
        private Network.EmbeddedHttpServer _httpServer;

        private void Awake()
        {
            if (Instance != null && Instance != this)
            {
                Destroy(gameObject);
                return;
            }

            Instance = this;
            DontDestroyOnLoad(gameObject);

            if (config == null)
            {
                config = ScriptableObject.CreateInstance<GAProfilerConfig>();
                Debug.LogWarning("[GAProfiler] No config assigned, using defaults.");
            }

            InitializeCollectors();
            Network.MainThreadDispatcher.EnsureInitialized();
            InitializeNetwork();
        }

        private void OnDestroy()
        {
            if (Instance == this)
            {
                if (State == CaptureState.Capturing)
                    StopCapture();
                _httpServer?.Stop();
                Instance = null;
            }
        }

        private void InitializeCollectors()
        {
            _frameCollector = new FrameDataCollector();
            _memoryCollector = new MemoryCollector();
            _renderingCollector = new RenderingCollector();
            _moduleTimingCollector = new ModuleTimingCollector();
            _jankDetector = new JankDetector(config.targetFps);
            _deviceMetricsCollector = new DeviceMetricsCollector();

            _screenshotCollector = new ScreenshotCollector();
            _screenshotCollector.Configure(
                config.screenshotInterval,
                config.screenshotThumbnailHeight,
                config.screenshotJpegQuality,
                config.fpsDropScreenshotThreshold,
                config.targetFps
            );
            _screenshotCollector.OnScreenshotCaptured += OnScreenshot;

            _overdrawCollector = new OverdrawCollector();
            _overdrawCollector.Configure(config.overdrawSampleInterval, config.overdrawShader);
            _overdrawCollector.OnOverdrawSampled += OnOverdraw;

            _functionTimingCollector = new FunctionTimingCollector();
            _logCollector = new LogCollector();

            _dataWriter = new ProfileDataWriter();
        }

        private void InitializeNetwork()
        {
            if (config.enableWifiTransfer)
            {
                _httpServer = new Network.EmbeddedHttpServer(config.httpServerPort, this);
                _httpServer.Start();
                Debug.Log($"[GAProfiler] HTTP server started on port {config.httpServerPort}");
            }
        }

        // ==================== Public API ====================

        /// <summary>Start capturing performance data.</summary>
        public void StartCapture(string sessionName = null)
        {
            if (State != CaptureState.Idle)
            {
                Debug.LogWarning("[GAProfiler] Already capturing or exporting.");
                return;
            }

            _session = new CaptureSession
            {
                sessionName = string.IsNullOrEmpty(sessionName)
                    ? $"Session_{DateTime.Now:yyyyMMdd_HHmmss}"
                    : sessionName,
                startTime = DateTime.Now,
                deviceInfo = DeviceMetricsCollector.CaptureDeviceInfo()
            };

            _captureStartTime = Time.realtimeSinceStartup;
            _framesSinceCapture = 0;
            _sampleEveryNFrames = Mathf.Max(1, config.sampleEveryNFrames);

            _session.deepProfilingEnabled = config.enableDeepProfiling;
            _session.targetFps = config.targetFps;

            State = CaptureState.Capturing;

            // Start all collectors
            _activeCollectors.Clear();

            _activeCollectors.Add(_frameCollector);

            if (config.enableMemory) _activeCollectors.Add(_memoryCollector);
            if (config.enableRendering) _activeCollectors.Add(_renderingCollector);
            if (config.enableModuleTiming) _activeCollectors.Add(_moduleTimingCollector);
            if (config.enableJankDetection) _activeCollectors.Add(_jankDetector);
            if (config.enableDeviceMetrics) _activeCollectors.Add(_deviceMetricsCollector);

            foreach (var c in _activeCollectors)
                c.OnCaptureStart();

            if (config.enableScreenshots)
                _screenshotCollector.OnCaptureStart();
            if (config.enableOverdraw)
                _overdrawCollector.OnCaptureStart();

            // Deep profiling
            if (config.enableDeepProfiling)
            {
                _functionTimingCollector.Initialize(_session);
                _functionTimingCollector.OnCaptureStart();
            }

            // Log capture
            if (config.captureLogs)
            {
                _logCollector.Initialize(_session, _captureStartTime);
                _logCollector.OnCaptureStart();
            }

            Debug.Log($"[GAProfiler] Capture started: {_session.sessionName} (deep={config.enableDeepProfiling})");
            OnCaptureStarted?.Invoke();
        }

        /// <summary>Stop capturing and save data.</summary>
        public void StopCapture()
        {
            if (State != CaptureState.Capturing) return;

            _session.duration = Time.realtimeSinceStartup - _captureStartTime;

            foreach (var c in _activeCollectors)
                c.OnCaptureStop();

            _screenshotCollector.OnCaptureStop();
            _overdrawCollector.OnCaptureStop();

            if (config.enableDeepProfiling)
                _functionTimingCollector.OnCaptureStop();
            if (config.captureLogs)
                _logCollector.OnCaptureStop();

            State = CaptureState.Exporting;
            Debug.Log($"[GAProfiler] Capture stopped. {_session.frames.Count} frames, {_session.duration:F1}s");

            StartCoroutine(ExportAndFinalize());
        }

        /// <summary>Get the last completed export path.</summary>
        public string LastExportPath { get; private set; }

        /// <summary>Get current session for network streaming.</summary>
        public CaptureSession CurrentSession => _session;

        /// <summary>Get latest frame data for live monitoring.</summary>
        public FrameData? LatestFrame { get; private set; }

        // ==================== Update Loop ====================

        private void Update()
        {
            if (State != CaptureState.Capturing) return;

            _framesSinceCapture++;

            // Sample every N frames
            if (_framesSinceCapture % _sampleEveryNFrames != 0) return;

            var frame = new FrameData();

            // Get current scene index
            string sceneName = SceneManager.GetActiveScene().name;
            frame.sceneIndex = _session.GetOrAddString(sceneName);

            // Run all collectors
            foreach (var c in _activeCollectors)
                c.Collect(ref frame);

            _session.frames.Add(frame);
            LatestFrame = frame;
            CurrentFps = frame.fps;
            _httpServer?.BroadcastFrame(frame);

            int frameIndex = _session.frames.Count - 1;

            // Deep profiling: collect function-level timing
            if (config.enableDeepProfiling && _framesSinceCapture % Mathf.Max(1, config.deepProfilingSampleRate) == 0)
            {
                var samples = _functionTimingCollector.CollectSamples();
                _session.frameFunctionSamples.Add(samples);
            }
            else
            {
                _session.frameFunctionSamples.Add(null); // placeholder for non-sampled frames
            }

            // Update log collector frame index
            if (config.captureLogs)
                _logCollector.UpdateFrameIndex(frameIndex);

            // Timed screenshots
            if (config.enableScreenshots)
            {
                if (_screenshotCollector.ShouldCaptureTimedScreenshot(frame.timestamp))
                {
                    StartCoroutine(_screenshotCollector.CaptureScreenshot(
                        frameIndex, frame.timestamp, ScreenshotTrigger.Timed));
                }
                else if (_screenshotCollector.ShouldCaptureAnomalyScreenshot(ref frame))
                {
                    var trigger = frame.jankLevel >= 2 ? ScreenshotTrigger.Jank : ScreenshotTrigger.FpsDrop;
                    StartCoroutine(_screenshotCollector.CaptureScreenshot(
                        frameIndex, frame.timestamp, trigger));
                }
            }

            // Overdraw sampling
            if (config.enableOverdraw && _overdrawCollector.ShouldSample(frame.timestamp))
            {
                var cam = Camera.main;
                if (cam != null)
                    _overdrawCollector.Sample(frameIndex, frame.timestamp, cam);
            }
        }

        // ==================== Callbacks ====================

        private void OnScreenshot(ScreenshotEntry entry)
        {
            _session?.screenshots.Add(entry);
        }

        private void OnOverdraw(OverdrawSample sample)
        {
            _session?.overdrawSamples.Add(sample);
        }

        // ==================== Export ====================

        private IEnumerator ExportAndFinalize()
        {
            string dir = System.IO.Path.Combine(
                Application.persistentDataPath, "GameAnalytics");
            if (!System.IO.Directory.Exists(dir))
                System.IO.Directory.CreateDirectory(dir);

            string fileName = $"{_session.sessionName}_{_session.deviceInfo.deviceModel}_{_session.startTime:yyyyMMdd_HHmmss}.gaprof";
            // Sanitize file name
            foreach (char c in System.IO.Path.GetInvalidFileNameChars())
                fileName = fileName.Replace(c, '_');

            string filePath = System.IO.Path.Combine(dir, fileName);

            Debug.Log($"[GAProfiler] Exporting to {filePath}...");

            // Write in chunks to avoid blocking
            int totalFrames = _session.frames.Count;
            int framesWritten = 0;

            yield return null; // Let UI update

            try
            {
                _dataWriter.Write(_session, filePath, (progress) =>
                {
                    OnExportProgress?.Invoke(progress);
                });
                LastExportPath = filePath;
                Debug.Log($"[GAProfiler] Export complete: {filePath} ({new System.IO.FileInfo(filePath).Length / 1024}KB)");
            }
            catch (Exception e)
            {
                Debug.LogError($"[GAProfiler] Export failed: {e.Message}");
                LastExportPath = null;
            }

            var exportInfo = new SessionExportInfo
            {
                filePath = LastExportPath,
                sessionName = _session.sessionName,
                frameCount = _session.frames.Count,
                duration = _session.duration,
                screenshotCount = _session.screenshots.Count
            };

            State = CaptureState.Idle;
            _session = null;
            OnCaptureStopped?.Invoke(exportInfo);
        }
    }

    public struct SessionExportInfo
    {
        public string filePath;
        public string sessionName;
        public int frameCount;
        public float duration;
        public int screenshotCount;
    }
}

#endif
