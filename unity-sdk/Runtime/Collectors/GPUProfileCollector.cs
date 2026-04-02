// GameAnalytics Device Profiler - GPU Profile Collector
// Captures GPU utilization metrics via FrameTimingManager.
// Provides GPU frame time, busy time, and pressure coefficient.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using UnityEngine;
using UnityEngine.Rendering;

namespace GameAnalytics.Profiler.Collectors
{
    /// <summary>
    /// Uses <see cref="FrameTimingManager"/> to capture GPU-side metrics:
    /// gpuFrameTime, GPU pressure coefficient (gpuFrameTime / budget),
    /// and CPU frequency for thermal context.
    /// </summary>
    public class GPUProfileCollector : ICollector
    {
        private int _targetFps;
        private float _gpuBudgetMs;
        private FrameTiming[] _timings = new FrameTiming[1];

        public GPUProfileCollector(int targetFps = 60)
        {
            _targetFps = Mathf.Max(1, targetFps);
            _gpuBudgetMs = 1000f / _targetFps;
        }

        public void OnCaptureStart()
        {
            // FrameTimingManager doesn't need explicit initialization
            // but we ensure at least one capture call to prime the system
            FrameTimingManager.CaptureFrameTimings();
        }

        public void Collect(ref Data.FrameData frame)
        {
            FrameTimingManager.CaptureFrameTimings();
            uint count = FrameTimingManager.GetLatestTimings(1, _timings);

            if (count > 0)
            {
                float gpuMs = (float)_timings[0].gpuFrameTime;
                frame.gpuTimeMs = gpuMs;
                frame.gpuUtilization = Mathf.Clamp01(gpuMs / _gpuBudgetMs);
                frame.cpuFrequencyMhz = (float)_timings[0].cpuMainThreadFrameTime; // cpu main thread time as proxy
            }
            else
            {
                // FrameTimingManager not available (some platforms)
                frame.gpuUtilization = 0f;
                frame.cpuFrequencyMhz = 0f;
            }
        }

        public void OnCaptureStop()
        {
            // Nothing to dispose
        }
    }
}

#endif
