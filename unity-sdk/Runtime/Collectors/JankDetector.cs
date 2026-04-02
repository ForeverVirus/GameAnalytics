// GameAnalytics Device Profiler - Jank Detector
// Identifies frame time spikes that indicate stuttering / hitches.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using UnityEngine;

namespace GameAnalytics.Profiler.Collectors
{
    public class JankDetector : ICollector
    {
        private float _targetFrameTimeMs;

        /// <summary>
        /// Constructor. targetFps e.g. 60 → target frame time = 16.67ms.
        /// Jank = >2x target, Severe = >4x target.
        /// </summary>
        public JankDetector(int targetFps = 60)
        {
            _targetFrameTimeMs = 1000f / targetFps;
        }

        public void SetTargetFps(int fps)
        {
            _targetFrameTimeMs = 1000f / fps;
        }

        public void OnCaptureStart() { }

        public void Collect(ref Data.FrameData frame)
        {
            float frameMs = frame.deltaTime * 1000f;
            if (frameMs > _targetFrameTimeMs * 4f)
                frame.jankLevel = 2; // Severe jank
            else if (frameMs > _targetFrameTimeMs * 2f)
                frame.jankLevel = 1; // Normal jank
            else
                frame.jankLevel = 0; // Smooth
        }

        public void OnCaptureStop() { }
    }
}

#endif
