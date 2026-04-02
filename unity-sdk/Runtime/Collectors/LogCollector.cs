// GameAnalytics Device Profiler - Log Collector
// Captures runtime Unity logs during profiling via Application.logMessageReceived.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using System.Collections.Generic;
using UnityEngine;
using GameAnalytics.Profiler.Data;

namespace GameAnalytics.Profiler.Collectors
{
    public class LogCollector
    {
        private CaptureSession _session;
        private float _captureStartTime;
        private int _currentFrame;
        private bool _isCapturing;
        private const int MaxLogEntries = 10000;

        public void Initialize(CaptureSession session, float captureStartTime)
        {
            _session = session;
            _captureStartTime = captureStartTime;
            _currentFrame = 0;
        }

        public void OnCaptureStart()
        {
            _isCapturing = true;
            Application.logMessageReceived += OnLogReceived;
        }

        public void UpdateFrameIndex(int frameIndex)
        {
            _currentFrame = frameIndex;
        }

        public void OnCaptureStop()
        {
            _isCapturing = false;
            Application.logMessageReceived -= OnLogReceived;
        }

        private void OnLogReceived(string message, string stackTrace, LogType type)
        {
            if (!_isCapturing || _session == null) return;
            if (_session.logEntries.Count >= MaxLogEntries) return;

            _session.logEntries.Add(new LogEntry
            {
                timestamp = Time.realtimeSinceStartup - _captureStartTime,
                message = message,
                stackTrace = stackTrace,
                logType = type,
                frameIndex = _currentFrame
            });
        }
    }
}

#endif
