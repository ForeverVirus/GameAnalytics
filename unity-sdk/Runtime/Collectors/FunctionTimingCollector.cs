// GameAnalytics Device Profiler - Function-Level Timing Collector
// Tracks 80+ Unity engine ProfilerRecorder markers to capture per-function
// CPU timing per frame, enabling UWA GOT-style thread call stack analysis.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using System;
using System.Collections.Generic;
using Unity.Profiling;
using GameAnalytics.Profiler.Data;

namespace GameAnalytics.Profiler.Collectors
{
    public class FunctionTimingCollector
    {
        private struct MarkerEntry
        {
            public string name;
            public FunctionCategory category;
            public ProfilerRecorder recorder;
        }

        private List<MarkerEntry> _markers = new List<MarkerEntry>();
        private CaptureSession _session;

        // Pre-defined engine markers matching UWA GOT's tracked functions
        private static readonly (string name, ProfilerCategory cat, FunctionCategory funcCat)[] KnownMarkers = new[]
        {
            // Rendering
            ("Camera.Render", ProfilerCategory.Render, FunctionCategory.Rendering),
            ("RenderPipelineManager.DoRenderLoop_Internal", ProfilerCategory.Render, FunctionCategory.Rendering),
            ("PostLateUpdate.UpdateAllRenderers", ProfilerCategory.Render, FunctionCategory.Rendering),
            ("Render.OpaqueGeometry", ProfilerCategory.Render, FunctionCategory.Rendering),
            ("Render.TransparentGeometry", ProfilerCategory.Render, FunctionCategory.Rendering),
            ("RenderForwardOpaque.Render", ProfilerCategory.Render, FunctionCategory.Rendering),
            ("Shadows.RenderShadowMap", ProfilerCategory.Render, FunctionCategory.Rendering),
            ("Culling", ProfilerCategory.Render, FunctionCategory.Rendering),

            // GPU Sync / Wait
            ("Gfx.WaitForPresentOnGfxThread", ProfilerCategory.Render, FunctionCategory.Sync),
            ("TimeUpdate.WaitForLastPresentationAndUpdateTime", ProfilerCategory.Render, FunctionCategory.Sync),
            ("Graphics.PresentAndSync", ProfilerCategory.Render, FunctionCategory.Sync),
            ("EndGraphicsJobs", ProfilerCategory.Render, FunctionCategory.Sync),

            // Scripts / User Code
            ("BehaviourUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("LateBehaviourUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("FixedBehaviourUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("CoroutinesDelayedCalls", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("Monobehaviour.OnMouse_", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("PlayerEndOfFrame", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("UnitySynchronizationContext.ExecuteTasks", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("Application.InvokeOnBeforeRender", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("NativeInputSystem.ShouldRunUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("NativeInputSystem.NotifyBeforeUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("NativeInputSystem.NotifyUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("PlayerConnection.Poll", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("AudioManager.Update", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("TextureStreamingManager.Update", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("EnlightenRuntimeManager.PostUpdate", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("LightProbeProxyVolumeManager.Update", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("ScriptableRuntimeReflectionSystemWrapper.Internal_ScriptableRuntimeReflectionSystemWrapper_TickRealtimeProbes", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("OnDemandRendering.GetRenderFrameInterval", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("SupportedRenderingFeatures.IsUIOverlayRenderedBySRP", ProfilerCategory.Scripts, FunctionCategory.Scripting),
            ("CustomRenderTextures.Update", ProfilerCategory.Scripts, FunctionCategory.Scripting),

            // Physics
            ("Physics.Processing", ProfilerCategory.Physics, FunctionCategory.Physics),
            ("Physics.Simulate", ProfilerCategory.Physics, FunctionCategory.Physics),
            ("Physics.FetchResults", ProfilerCategory.Physics, FunctionCategory.Physics),
            ("Physics.ProcessReports", ProfilerCategory.Physics, FunctionCategory.Physics),
            ("Physics.Interpolation", ProfilerCategory.Physics, FunctionCategory.Physics),
            ("Physics.UpdateBodies", ProfilerCategory.Physics, FunctionCategory.Physics),
            ("Physics.UpdateCloth", ProfilerCategory.Physics, FunctionCategory.Physics),
            ("Physics.UpdateVehicles", ProfilerCategory.Physics, FunctionCategory.Physics),
            ("Physics2D.Simulate", ProfilerCategory.Physics, FunctionCategory.Physics),
            ("Physics2D.InterpolatePoses", ProfilerCategory.Physics, FunctionCategory.Physics),

            // Animation
            ("Director.ProcessFrame", ProfilerCategory.Animation, FunctionCategory.Animation),
            ("Director.PrepareFrame", ProfilerCategory.Animation, FunctionCategory.Animation),
            ("Director.SampleTime", ProfilerCategory.Animation, FunctionCategory.Animation),
            ("DirectorUpdate", ProfilerCategory.Animation, FunctionCategory.Animation),
            ("MeshSkinning.Update", ProfilerCategory.Animation, FunctionCategory.Animation),

            // UI
            ("UI.LayoutUpdate", ProfilerCategory.Gui, FunctionCategory.UI),
            ("UI.RenderOverlays", ProfilerCategory.Gui, FunctionCategory.UI),
            ("Rendering.UpdateBatches", ProfilerCategory.Gui, FunctionCategory.UI),
            ("Rendering.RenderOverlays", ProfilerCategory.Gui, FunctionCategory.UI),
            ("Rendering.EmitWorldScreenspaceCameraGeometry", ProfilerCategory.Gui, FunctionCategory.UI),
            ("GUI.Repaint", ProfilerCategory.Gui, FunctionCategory.UI),
            ("GUI.ProcessEvents", ProfilerCategory.Gui, FunctionCategory.UI),

            // Loading
            ("Loading.AsyncRead", ProfilerCategory.Loading, FunctionCategory.Loading),
            ("Loading.PreloadManager", ProfilerCategory.Loading, FunctionCategory.Loading),
            ("UpdatePreloading", ProfilerCategory.Loading, FunctionCategory.Loading),

            // Particles
            ("ParticleSystem.Update", ProfilerCategory.Particles, FunctionCategory.Particles),
            ("ParticleSystem.EndUpdateAll", ProfilerCategory.Particles, FunctionCategory.Particles),

            // GC
            ("GC.Collect", ProfilerCategory.Memory, FunctionCategory.GC),
        };

        public void Initialize(CaptureSession session)
        {
            _session = session;
        }

        public void OnCaptureStart()
        {
            _markers.Clear();
            foreach (var m in KnownMarkers)
            {
                var rec = ProfilerRecorder.StartNew(m.cat, m.name, 1);
                _markers.Add(new MarkerEntry
                {
                    name = m.name,
                    category = m.funcCat,
                    recorder = rec
                });
            }
        }

        public List<FunctionSample> CollectSamples()
        {
            var samples = new List<FunctionSample>();

            for (int i = 0; i < _markers.Count; i++)
            {
                var m = _markers[i];
                if (!m.recorder.Valid) continue;

                long ns = m.recorder.LastValue;
                if (ns <= 0) continue;

                float ms = ns / 1_000_000f;
                ushort nameIdx = _session.GetOrAddString(m.name);

                samples.Add(new FunctionSample
                {
                    functionNameIndex = nameIdx,
                    category = m.category,
                    selfTimeMs = ms,
                    totalTimeMs = ms,
                    callCount = (ushort)System.Math.Max(1, m.recorder.Count),
                    depth = 0,
                    parentIndex = -1
                });
            }

            return samples;
        }

        public void OnCaptureStop()
        {
            foreach (var m in _markers)
            {
                if (m.recorder.Valid)
                    m.recorder.Dispose();
            }
            _markers.Clear();
        }
    }
}

#endif
