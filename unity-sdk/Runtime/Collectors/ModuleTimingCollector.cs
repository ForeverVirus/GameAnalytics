// GameAnalytics Device Profiler - Module Timing Collector
// Uses ProfilerRecorder to capture per-module CPU time breakdown.
// Markers vary between Built-in and SRP pipelines.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using Unity.Profiling;

namespace GameAnalytics.Profiler.Collectors
{
    public class ModuleTimingCollector : ICollector
    {
        // Rendering
        private ProfilerRecorder _cameraRender;
        private ProfilerRecorder _srpRender;

        // Scripts
        private ProfilerRecorder _scriptsUpdate;
        private ProfilerRecorder _scriptsLateUpdate;
        private ProfilerRecorder _scriptsFixedUpdate;

        // Physics
        private ProfilerRecorder _physics;

        // Animation
        private ProfilerRecorder _animation;

        // UI
        private ProfilerRecorder _uiLayout;
        private ProfilerRecorder _uiRender;

        // Particles
        private ProfilerRecorder _particles;

        // Loading
        private ProfilerRecorder _asyncRead;
        private ProfilerRecorder _preloadManager;

        // GC
        private ProfilerRecorder _gcCollect;

        public void OnCaptureStart()
        {
            // Try both built-in and SRP render markers
            _cameraRender = ProfilerRecorder.StartNew(ProfilerCategory.Render, "Camera.Render");
            _srpRender = ProfilerRecorder.StartNew(ProfilerCategory.Render, "RenderPipelineManager.DoRenderLoop_Internal");

            _scriptsUpdate = ProfilerRecorder.StartNew(ProfilerCategory.Scripts, "BehaviourUpdate");
            _scriptsLateUpdate = ProfilerRecorder.StartNew(ProfilerCategory.Scripts, "LateBehaviourUpdate");
            _scriptsFixedUpdate = ProfilerRecorder.StartNew(ProfilerCategory.Scripts, "FixedBehaviourUpdate");

            _physics = ProfilerRecorder.StartNew(ProfilerCategory.Physics, "Physics.Simulate");

            _animation = ProfilerRecorder.StartNew(ProfilerCategory.Animation, "DirectorUpdate");

            _uiLayout = ProfilerRecorder.StartNew(ProfilerCategory.Gui, "UI.LayoutUpdate");
            _uiRender = ProfilerRecorder.StartNew(ProfilerCategory.Gui, "UI.RenderOverlays");

            _particles = ProfilerRecorder.StartNew(ProfilerCategory.Particles, "ParticleSystem.Update");

            _asyncRead = ProfilerRecorder.StartNew(ProfilerCategory.Loading, "Loading.AsyncRead");
            _preloadManager = ProfilerRecorder.StartNew(ProfilerCategory.Loading, "Loading.PreloadManager");

            _gcCollect = ProfilerRecorder.StartNew(ProfilerCategory.Memory, "GC.Collect");
        }

        public void Collect(ref Data.FrameData frame)
        {
            // Rendering: pick whichever marker is active (built-in vs SRP)
            long renderNs = GetNs(_cameraRender);
            if (renderNs == 0) renderNs = GetNs(_srpRender);
            frame.renderTime = NsToMs(renderNs);

            frame.scriptsUpdateTime = NsToMs(GetNs(_scriptsUpdate));
            frame.scriptsLateUpdateTime = NsToMs(GetNs(_scriptsLateUpdate));
            frame.fixedUpdateTime = NsToMs(GetNs(_scriptsFixedUpdate));

            frame.physicsTime = NsToMs(GetNs(_physics));
            frame.animationTime = NsToMs(GetNs(_animation));

            long uiNs = GetNs(_uiLayout) + GetNs(_uiRender);
            frame.uiTime = NsToMs(uiNs);

            frame.particleTime = NsToMs(GetNs(_particles));

            long loadNs = GetNs(_asyncRead) + GetNs(_preloadManager);
            frame.loadingTime = NsToMs(loadNs);

            frame.gcCollectTime = NsToMs(GetNs(_gcCollect));

            // Compute "other" as frame time minus known modules
            float known = frame.renderTime + frame.scriptsUpdateTime +
                          frame.scriptsLateUpdateTime + frame.fixedUpdateTime +
                          frame.physicsTime + frame.animationTime +
                          frame.uiTime + frame.particleTime +
                          frame.loadingTime + frame.gcCollectTime;
            float total = frame.deltaTime * 1000f;
            frame.otherTime = total > known ? total - known : 0f;

            // Render submit (part of render, but from command buffer)
            frame.renderSubmitTime = 0f; // Will be populated by FrameTimingManager if available
        }

        public void OnCaptureStop()
        {
            _cameraRender.Dispose();
            _srpRender.Dispose();
            _scriptsUpdate.Dispose();
            _scriptsLateUpdate.Dispose();
            _scriptsFixedUpdate.Dispose();
            _physics.Dispose();
            _animation.Dispose();
            _uiLayout.Dispose();
            _uiRender.Dispose();
            _particles.Dispose();
            _asyncRead.Dispose();
            _preloadManager.Dispose();
            _gcCollect.Dispose();
        }

        private static long GetNs(ProfilerRecorder rec)
        {
            return rec.Valid ? rec.LastValue : 0;
        }

        private static float NsToMs(long ns)
        {
            return ns / 1_000_000f;
        }
    }
}

#endif
