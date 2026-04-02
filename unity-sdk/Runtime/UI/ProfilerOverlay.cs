// GameAnalytics Device Profiler - Runtime UI Overlay
// Draggable floating FPS display + expandable control panel.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using UnityEngine;

namespace GameAnalytics.Profiler.UI
{
    public class ProfilerOverlay : MonoBehaviour
    {
        private bool _expanded;
        private bool _dragging;
        private Vector2 _dragOffset;
        private Rect _buttonRect = new Rect(10, 10, 80, 36);
        private Rect _panelRect;
        private string _sessionName = "";
        private Vector2 _scrollPos;

        private GUIStyle _buttonStyle;
        private GUIStyle _labelStyle;
        private GUIStyle _boxStyle;
        private GUIStyle _headerStyle;
        private bool _stylesInitialized;

        private void InitStyles()
        {
            if (_stylesInitialized) return;
            _stylesInitialized = true;

            _buttonStyle = new GUIStyle(GUI.skin.button)
            {
                fontSize = 14,
                fontStyle = FontStyle.Bold
            };

            _labelStyle = new GUIStyle(GUI.skin.label)
            {
                fontSize = 12,
                richText = true
            };

            _boxStyle = new GUIStyle(GUI.skin.box)
            {
                padding = new RectOffset(8, 8, 8, 8)
            };

            _headerStyle = new GUIStyle(GUI.skin.label)
            {
                fontSize = 14,
                fontStyle = FontStyle.Bold,
                alignment = TextAnchor.MiddleCenter
            };
        }

        private void OnGUI()
        {
            if (GAProfiler.Instance == null) return;

            InitStyles();

            var profiler = GAProfiler.Instance;

            // FPS color
            float fps = profiler.CurrentFps;
            Color fpsColor = fps >= 55 ? Color.green : fps >= 30 ? Color.yellow : Color.red;
            string fpsText = fps > 0 ? $"{fps:F0}" : "--";

            // Draggable FPS button
            GUI.backgroundColor = new Color(0, 0, 0, 0.7f);

            // Handle drag
            var e = Event.current;
            if (e.type == EventType.MouseDown && _buttonRect.Contains(e.mousePosition))
            {
                if (e.button == 0)
                {
                    _dragging = true;
                    _dragOffset = e.mousePosition - new Vector2(_buttonRect.x, _buttonRect.y);
                    e.Use();
                }
            }
            else if (e.type == EventType.MouseDrag && _dragging)
            {
                _buttonRect.x = e.mousePosition.x - _dragOffset.x;
                _buttonRect.y = e.mousePosition.y - _dragOffset.y;
                e.Use();
            }
            else if (e.type == EventType.MouseUp && _dragging)
            {
                _dragging = false;

                // If barely moved, treat as click
                Vector2 moved = e.mousePosition - new Vector2(_buttonRect.x + _dragOffset.x, _buttonRect.y + _dragOffset.y);
                if (moved.magnitude < 5f)
                    _expanded = !_expanded;
            }

            // FPS button
            var origColor = GUI.contentColor;
            GUI.contentColor = fpsColor;
            if (GUI.Button(_buttonRect, $"FPS {fpsText}", _buttonStyle))
            {
                if (!_dragging) _expanded = !_expanded;
            }
            GUI.contentColor = origColor;

            // Expanded panel
            if (_expanded)
            {
                _panelRect = new Rect(_buttonRect.x, _buttonRect.yMax + 4, 280, 360);

                // Clamp to screen
                _panelRect.x = Mathf.Clamp(_panelRect.x, 0, Screen.width - _panelRect.width);
                _panelRect.y = Mathf.Clamp(_panelRect.y, 0, Screen.height - _panelRect.height);

                GUI.Box(_panelRect, "", _boxStyle);
                GUILayout.BeginArea(new Rect(_panelRect.x + 8, _panelRect.y + 8,
                                              _panelRect.width - 16, _panelRect.height - 16));

                GUILayout.Label("GameAnalytics Profiler", _headerStyle);
                GUILayout.Space(4);

                // State indicator
                string stateText = profiler.State == CaptureState.Capturing
                    ? $"<color=red>● Recording</color> ({profiler.CapturedFrameCount} frames, {profiler.CaptureElapsed:F1}s)"
                    : profiler.State == CaptureState.Exporting
                    ? "<color=yellow>⟳ Exporting...</color>"
                    : "<color=white>○ Idle</color>";
                GUILayout.Label(stateText, _labelStyle);
                GUILayout.Space(4);

                // Live metrics
                var frame = profiler.LatestFrame;
                if (frame.HasValue)
                {
                    var f = frame.Value;
                    GUILayout.Label($"CPU: <color=cyan>{f.cpuTimeMs:F1}ms</color>  GPU: <color=cyan>{f.gpuTimeMs:F1}ms</color>", _labelStyle);
                    GUILayout.Label($"Memory: <color=cyan>{f.totalAllocated / (1024f * 1024f):F1}MB</color>  GFX: <color=cyan>{f.gfxMemory / (1024f * 1024f):F1}MB</color>", _labelStyle);
                    GUILayout.Label($"DC: <color=cyan>{f.drawCalls}</color>  Batches: <color=cyan>{f.batches}</color>  Tris: <color=cyan>{f.triangles / 1000}K</color>", _labelStyle);

                    if (f.batteryLevel > 0)
                        GUILayout.Label($"Battery: <color=cyan>{f.batteryLevel * 100:F0}%</color>  Temp: <color=cyan>{f.temperature:F1}°C</color>", _labelStyle);

                    if (f.jankLevel > 0)
                        GUILayout.Label(f.jankLevel >= 2
                            ? "<color=red>⚠ SEVERE JANK</color>"
                            : "<color=yellow>⚠ Jank detected</color>", _labelStyle);
                }

                GUILayout.Space(8);

                // Controls
                if (profiler.State == CaptureState.Idle)
                {
                    GUILayout.Label("Session Name:", _labelStyle);
                    _sessionName = GUILayout.TextField(_sessionName);
                    GUILayout.Space(4);

                    GUI.backgroundColor = new Color(0.2f, 0.8f, 0.2f, 0.9f);
                    if (GUILayout.Button("▶ Start Capture", GUILayout.Height(32)))
                    {
                        profiler.StartCapture(string.IsNullOrEmpty(_sessionName) ? null : _sessionName);
                    }
                }
                else if (profiler.State == CaptureState.Capturing)
                {
                    GUI.backgroundColor = new Color(0.9f, 0.2f, 0.2f, 0.9f);
                    if (GUILayout.Button("■ Stop & Export", GUILayout.Height(32)))
                    {
                        profiler.StopCapture();
                    }
                }

                GUI.backgroundColor = new Color(0, 0, 0, 0.7f);

                // Last export info
                if (!string.IsNullOrEmpty(profiler.LastExportPath))
                {
                    GUILayout.Space(4);
                    GUILayout.Label($"<color=green>✓ Saved:</color> {System.IO.Path.GetFileName(profiler.LastExportPath)}", _labelStyle);
                }

                // WiFi info
                var config = profiler.config;
                if (config != null && config.enableWifiTransfer)
                {
                    GUILayout.Space(4);
                    string ip = GetLocalIP();
                    GUILayout.Label($"<color=cyan>WiFi:</color> {ip}:{config.httpServerPort}", _labelStyle);
                }

                GUILayout.EndArea();
            }

            GUI.backgroundColor = Color.white;
        }

        private static string GetLocalIP()
        {
            try
            {
                var host = System.Net.Dns.GetHostEntry(System.Net.Dns.GetHostName());
                foreach (var ip in host.AddressList)
                {
                    if (ip.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork
                        && !System.Net.IPAddress.IsLoopback(ip))
                        return ip.ToString();
                }
            }
            catch { }
            return "127.0.0.1";
        }
    }
}

#endif
