// GameAnalytics Device Profiler - Overdraw Collector
// Renders scene with overdraw visualization shader and measures pixel overlap.
// Performance-heavy: should be sampled infrequently (every 30s or manual trigger only).

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using UnityEngine;
using UnityEngine.Rendering;

namespace GameAnalytics.Profiler.Collectors
{
    public class OverdrawCollector
    {
        private Shader _overdrawShader;
        private Material _overdrawMaterial;
        private RenderTexture _overdrawRT;
        private Texture2D _readbackTex;
        private float _sampleInterval = 30f;
        private float _lastSampleTime;
        private bool _capturing;
        private int _resolution = 256; // Low-res for analysis

        public System.Action<Data.OverdrawSample> OnOverdrawSampled;

        public void Configure(float sampleInterval, Shader overdrawShader)
        {
            _sampleInterval = sampleInterval;
            _overdrawShader = overdrawShader;
        }

        public void OnCaptureStart()
        {
            _lastSampleTime = -_sampleInterval;
            _capturing = true;

            if (_overdrawShader == null)
            {
                _overdrawShader = Shader.Find("Hidden/GAProfiler/Overdraw");
            }

            if (_overdrawShader != null)
            {
                _overdrawMaterial = new Material(_overdrawShader);
            }

            _overdrawRT = new RenderTexture(_resolution, _resolution, 16, RenderTextureFormat.ARGB32);
            _readbackTex = new Texture2D(_resolution, _resolution, TextureFormat.RGBA32, false);
        }

        public bool ShouldSample(float currentTime)
        {
            if (!_capturing || _overdrawShader == null) return false;
            return currentTime - _lastSampleTime >= _sampleInterval;
        }

        /// <summary>
        /// Performs overdraw analysis. Call from the main camera's rendering context.
        /// Returns the overdraw sample, or null if unavailable.
        /// </summary>
        public Data.OverdrawSample Sample(int frameIndex, float timestamp, Camera camera)
        {
            if (!_capturing || camera == null || _overdrawMaterial == null) return null;

            _lastSampleTime = timestamp;

            // Save camera state
            var prevRT = camera.targetTexture;
            var prevClearFlags = camera.clearFlags;
            var prevBg = camera.backgroundColor;

            // Render with overdraw shader replacement
            camera.targetTexture = _overdrawRT;
            camera.clearFlags = CameraClearFlags.SolidColor;
            camera.backgroundColor = Color.black;
            camera.RenderWithShader(_overdrawShader, "");

            // Restore
            camera.targetTexture = prevRT;
            camera.clearFlags = prevClearFlags;
            camera.backgroundColor = prevBg;

            // Read back pixels
            var prevActive = RenderTexture.active;
            RenderTexture.active = _overdrawRT;
            _readbackTex.ReadPixels(new Rect(0, 0, _resolution, _resolution), 0, 0, false);
            _readbackTex.Apply(false);
            RenderTexture.active = prevActive;

            // Calculate average overdraw from alpha channel
            // Each pixel's red channel represents number of times it was drawn
            // (shader outputs additive 1/255 per draw, so value * 255 = layer count)
            var pixels = _readbackTex.GetPixels32();
            long totalLayers = 0;
            int validPixels = 0;
            for (int i = 0; i < pixels.Length; i++)
            {
                int layers = pixels[i].r; // Each draw adds ~1 to red channel
                if (layers > 0)
                {
                    totalLayers += layers;
                    validPixels++;
                }
            }

            float avgOverdraw = validPixels > 0 ? (float)totalLayers / validPixels : 0f;

            // Encode heatmap as JPEG
            byte[] heatmapJpeg = _readbackTex.EncodeToJPG(60);

            var sample = new Data.OverdrawSample
            {
                frameIndex = frameIndex,
                timestamp = timestamp,
                avgOverdrawLayers = avgOverdraw,
                heatmapJpeg = heatmapJpeg
            };

            OnOverdrawSampled?.Invoke(sample);
            return sample;
        }

        public void OnCaptureStop()
        {
            _capturing = false;
            if (_overdrawRT != null) { _overdrawRT.Release(); Object.Destroy(_overdrawRT); }
            if (_readbackTex != null) Object.Destroy(_readbackTex);
            if (_overdrawMaterial != null) Object.Destroy(_overdrawMaterial);
        }
    }
}

#endif
