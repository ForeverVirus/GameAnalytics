// GameAnalytics Device Profiler - Screenshot Collector
// Captures screenshots at timed intervals and on performance anomalies.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using System.Collections;
using UnityEngine;

namespace GameAnalytics.Profiler.Collectors
{
    public class ScreenshotCollector
    {
        private float _interval = 5f;
        private float _lastCaptureTime;
        private int _thumbnailHeight = 480;
        private int _jpegQuality = 75;
        private float _fpsDropThreshold = 0.6f; // Trigger if FPS < targetFPS * this
        private int _targetFps = 60;
        private bool _capturing;

        // Callbacks
        public System.Action<Data.ScreenshotEntry> OnScreenshotCaptured;

        public void Configure(float interval, int thumbnailHeight, int jpegQuality,
                              float fpsDropThreshold, int targetFps)
        {
            _interval = interval;
            _thumbnailHeight = thumbnailHeight;
            _jpegQuality = jpegQuality;
            _fpsDropThreshold = fpsDropThreshold;
            _targetFps = targetFps;
        }

        public void OnCaptureStart()
        {
            _lastCaptureTime = -_interval; // Capture immediately
            _capturing = true;
        }

        public void OnCaptureStop()
        {
            _capturing = false;
        }

        /// <summary>
        /// Call each frame to check if a timed screenshot is due.
        /// Returns true if a screenshot should be queued via coroutine.
        /// </summary>
        public bool ShouldCaptureTimedScreenshot(float currentTime)
        {
            if (!_capturing) return false;
            return currentTime - _lastCaptureTime >= _interval;
        }

        /// <summary>
        /// Check if current metrics warrant an anomaly-triggered screenshot.
        /// </summary>
        public bool ShouldCaptureAnomalyScreenshot(ref Data.FrameData frame)
        {
            if (!_capturing) return false;

            // Don't take anomaly screenshots more than once per second
            if (frame.timestamp - _lastCaptureTime < 1f) return false;

            // FPS drop below threshold
            if (frame.fps > 0 && frame.fps < _targetFps * _fpsDropThreshold)
                return true;

            // Severe jank
            if (frame.jankLevel >= 2)
                return true;

            return false;
        }

        /// <summary>
        /// Coroutine that actually captures the screenshot at end of frame.
        /// Must be started from a MonoBehaviour.
        /// </summary>
        public IEnumerator CaptureScreenshot(int frameIndex, float timestamp,
                                              Data.ScreenshotTrigger trigger)
        {
            yield return new WaitForEndOfFrame();

            if (!_capturing) yield break;

            _lastCaptureTime = timestamp;

            // Capture full screen
            var screenTex = ScreenCapture.CaptureScreenshotAsTexture();
            if (screenTex == null) yield break;

            // Downscale to thumbnail
            int srcW = screenTex.width;
            int srcH = screenTex.height;
            int dstH = Mathf.Min(_thumbnailHeight, srcH);
            int dstW = Mathf.RoundToInt((float)srcW / srcH * dstH);

            var thumbRT = RenderTexture.GetTemporary(dstW, dstH, 0, RenderTextureFormat.ARGB32);
            Graphics.Blit(screenTex, thumbRT);

            var prevActive = RenderTexture.active;
            RenderTexture.active = thumbRT;
            var thumbTex = new Texture2D(dstW, dstH, TextureFormat.RGB24, false);
            thumbTex.ReadPixels(new Rect(0, 0, dstW, dstH), 0, 0, false);
            thumbTex.Apply(false);
            RenderTexture.active = prevActive;

            // Encode to JPEG
            byte[] jpegData = thumbTex.EncodeToJPG(_jpegQuality);

            // Cleanup
            Object.Destroy(screenTex);
            Object.Destroy(thumbTex);
            RenderTexture.ReleaseTemporary(thumbRT);

            if (jpegData != null && jpegData.Length > 0)
            {
                var entry = new Data.ScreenshotEntry
                {
                    frameIndex = frameIndex,
                    timestamp = timestamp,
                    jpegData = jpegData,
                    trigger = trigger
                };
                OnScreenshotCaptured?.Invoke(entry);
            }
        }
    }
}

#endif
