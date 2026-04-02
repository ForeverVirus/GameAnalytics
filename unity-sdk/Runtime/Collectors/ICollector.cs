// GameAnalytics Device Profiler - Base Collector Interface

#if DEVELOPMENT_BUILD || UNITY_EDITOR

namespace GameAnalytics.Profiler.Collectors
{
    /// <summary>
    /// Base interface for all profiler data collectors.
    /// Each collector is responsible for populating specific fields of FrameData.
    /// </summary>
    public interface ICollector
    {
        /// <summary>Called once when capture starts. Initialize ProfilerRecorders etc.</summary>
        void OnCaptureStart();

        /// <summary>Called each sampled frame. Write data into the FrameData ref.</summary>
        void Collect(ref Data.FrameData frame);

        /// <summary>Called once when capture stops. Dispose recorders.</summary>
        void OnCaptureStop();
    }
}

#endif
