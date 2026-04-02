// GameAnalytics Device Profiler - Embedded HTTP Server
// Lightweight HTTP server for WiFi data transfer to desktop.
// Uses TcpListener instead of HttpListener for better mobile compatibility.

#if DEVELOPMENT_BUILD || UNITY_EDITOR

using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Net;
using System.Net.Sockets;
using System.Reflection;
using System.Text;
using System.Threading;
using UnityEngine;

namespace GameAnalytics.Profiler.Network
{
    public class EmbeddedHttpServer
    {
        private readonly int _port;
        private readonly GAProfiler _profiler;
        private readonly string _sessionsDirectory;
        private TcpListener _listener;
        private Thread _listenerThread;
        private volatile bool _running;
        private readonly List<SseClient> _sseClients = new List<SseClient>();
        private readonly object _sseLock = new object();

        public bool IsRunning => _running;
        public int Port => _port;

        public EmbeddedHttpServer(int port, GAProfiler profiler, string sessionsDirectory)
        {
            _port = port;
            _profiler = profiler;
            _sessionsDirectory = sessionsDirectory;
        }

        public void Start()
        {
            if (_running) return;

            try
            {
                Stop();
                _listener = new TcpListener(IPAddress.Any, _port);
                _listener.Server.SetSocketOption(SocketOptionLevel.Socket, SocketOptionName.ReuseAddress, true);
                _listener.Start();
                _running = true;

                _listenerThread = new Thread(ListenLoop)
                {
                    IsBackground = true,
                    Name = "GAProfiler-TCPHTTP"
                };
                _listenerThread.Start();
            }
            catch (Exception e)
            {
                Debug.LogError($"[GAProfiler] Failed to start TCP HTTP server on port {_port}: {e.Message}");
                _running = false;
            }
        }

        public void Stop()
        {
            _running = false;

            try { _listener?.Stop(); } catch { }
            try { _listener = null; } catch { }
            try
            {
                if (_listenerThread != null && _listenerThread.IsAlive)
                    _listenerThread.Join(200);
            }
            catch { }
            finally
            {
                _listenerThread = null;
            }

            lock (_sseLock)
            {
                foreach (var client in _sseClients)
                {
                    try { client.Writer.Dispose(); } catch { }
                    try { client.Client.Close(); } catch { }
                }
                _sseClients.Clear();
            }
        }

        public void BroadcastFrame(Data.FrameData frame)
        {
            if (!_running) return;

            string json = JsonUtility.ToJson(frame);
            string payload = $"data: {json}\n\n";

            lock (_sseLock)
            {
                for (int i = _sseClients.Count - 1; i >= 0; i--)
                {
                    try
                    {
                        _sseClients[i].Writer.Write(payload);
                        _sseClients[i].Writer.Flush();
                    }
                    catch
                    {
                        try { _sseClients[i].Writer.Dispose(); } catch { }
                        try { _sseClients[i].Client.Close(); } catch { }
                        _sseClients.RemoveAt(i);
                    }
                }
            }
        }

        private void ListenLoop()
        {
            while (_running && _listener != null)
            {
                try
                {
                    var client = _listener.AcceptTcpClient();
                    ThreadPool.QueueUserWorkItem(_ => HandleClient(client));
                }
                catch (SocketException)
                {
                    if (!_running) break;
                }
                catch (ObjectDisposedException)
                {
                    break;
                }
                catch (Exception e)
                {
                    if (_running)
                    {
                        Debug.LogError($"[GAProfiler] TCP HTTP error: {e.Message}");
                    }
                }
            }
        }

        private void HandleClient(TcpClient client)
        {
            NetworkStream stream = null;
            StreamReader reader = null;
            bool keepAlive = false;
            try
            {
                stream = client.GetStream();
                reader = new StreamReader(stream, Encoding.UTF8, false, 8192, true);

                var requestLine = reader.ReadLine();
                if (string.IsNullOrEmpty(requestLine))
                {
                    return;
                }

                var parts = requestLine.Split(' ');
                if (parts.Length < 2)
                {
                    WriteJson(stream, 400, "{\"error\":\"Invalid request\"}");
                    return;
                }

                var method = parts[0].ToUpperInvariant();
                var path = parts[1];
                int contentLength = 0;

                string line;
                while (!string.IsNullOrEmpty(line = reader.ReadLine()))
                {
                    int colon = line.IndexOf(':');
                    if (colon <= 0) continue;
                    var headerName = line.Substring(0, colon).Trim();
                    var headerValue = line.Substring(colon + 1).Trim();
                    if (headerName.Equals("Content-Length", StringComparison.OrdinalIgnoreCase))
                    {
                        int.TryParse(headerValue, out contentLength);
                    }
                }

                string body = string.Empty;
                const int MaxBodySize = 1024 * 1024; // 1MB max
                if (contentLength > MaxBodySize)
                {
                    WriteJson(stream, 413, "{\"error\":\"Payload too large\"}");
                    return;
                }
                if (contentLength > 0)
                {
                    var buffer = new char[contentLength];
                    int read = 0;
                    while (read < contentLength)
                    {
                        int chunk = reader.Read(buffer, read, contentLength - read);
                        if (chunk <= 0) break;
                        read += chunk;
                    }
                    body = new string(buffer, 0, read);
                }

                keepAlive = HandleRoute(client, stream, method, path, body);
            }
            catch (Exception e)
            {
                try
                {
                    if (stream != null)
                    {
                        WriteJson(stream, 500, $"{{\"error\":\"{EscapeJson(e.Message)}\"}}");
                    }
                }
                catch
                {
                    // Ignore secondary write failure.
                }
            }
            finally
            {
                if (!keepAlive)
                {
                    try { reader?.Dispose(); } catch { }
                    try { stream?.Dispose(); } catch { }
                    try { client.Close(); } catch { }
                }
            }
        }

        private bool HandleRoute(TcpClient client, NetworkStream stream, string method, string path, string body)
        {
            string normalizedPath = path.TrimEnd('/');

            if (method == "GET" && normalizedPath == "/status")
            {
                HandleStatus(stream);
                return false;
            }
            else if (method == "GET" && normalizedPath == "/live")
            {
                HandleLiveSse(client, stream);
                return true;
            }
            else if (method == "GET" && normalizedPath == "/sessions")
            {
                HandleListSessions(stream);
                return false;
            }
            else if (method == "GET" && normalizedPath.StartsWith("/sessions/") && normalizedPath.EndsWith("/download"))
            {
                HandleDownloadSession(stream, normalizedPath);
                return false;
            }
            else if (method == "POST" && normalizedPath == "/capture/start")
            {
                HandleStartCapture(stream, body);
                return false;
            }
            else if (method == "POST" && normalizedPath == "/capture/stop")
            {
                HandleStopCapture(stream);
                return false;
            }
            else if (method == "POST" && normalizedPath == "/deep-profiling/toggle")
            {
                HandleDeepProfilingToggle(stream, body);
                return false;
            }
            else if (method == "GET" && normalizedPath.StartsWith("/sessions/") && normalizedPath.EndsWith("/deep-download"))
            {
                HandleDeepDataDownload(stream, normalizedPath);
                return false;
            }
            else
            {
                WriteJson(stream, 404, "{\"error\":\"Not found\"}");
                return false;
            }
        }

        private void HandleStatus(NetworkStream stream)
        {
            StatusResponse status = null;
            using (var done = new ManualResetEvent(false))
            {
                MainThreadDispatcher.Enqueue(() =>
                {
                    try
                    {
                        status = new StatusResponse
                        {
                            deviceModel = SystemInfo.deviceModel,
                            projectName = Application.productName,
                            sdkVersion = "1.0.0",
                            capturing = _profiler.State == CaptureState.Capturing,
                            frameCount = _profiler.CapturedFrameCount,
                            elapsed = _profiler.CaptureElapsed,
                            currentFps = _profiler.CurrentFps,
                            deepCaptureEnabled = _profiler.config != null && _profiler.config.enableDeepCapture,
                            hasDeepData = _profiler.HasDeepProfileData,
                            deepDataSize = _profiler.DeepProfileDataSize
                        };
                    }
                    finally
                    {
                        done.Set();
                    }
                });

                if (!done.WaitOne(1000))
                {
                    WriteJson(stream, 504, "{\"error\":\"Status request timed out\"}");
                    return;
                }
            }

            if (status == null)
            {
                WriteJson(stream, 500, "{\"error\":\"Failed to build status snapshot\"}");
                return;
            }

            WriteJson(stream, 200, JsonUtility.ToJson(status));
        }

        private void HandleLiveSse(TcpClient client, NetworkStream stream)
        {
            string headers =
                "HTTP/1.1 200 OK\r\n" +
                "Content-Type: text/event-stream\r\n" +
                "Cache-Control: no-cache\r\n" +
                "Connection: keep-alive\r\n" +
                "Access-Control-Allow-Origin: *\r\n\r\n";
            byte[] headerBytes = Encoding.UTF8.GetBytes(headers);
            stream.Write(headerBytes, 0, headerBytes.Length);
            stream.Flush();

            var writer = new StreamWriter(stream, Encoding.UTF8, 1024, true)
            {
                AutoFlush = true
            };
            writer.Write("event: connected\ndata: {\"status\":\"ok\"}\n\n");

            lock (_sseLock)
            {
                _sseClients.Add(new SseClient
                {
                    Client = client,
                    Writer = writer
                });
            }
        }

        private void HandleListSessions(NetworkStream stream)
        {
            var sessions = new List<SessionListItem>();

            if (Directory.Exists(_sessionsDirectory))
            {
                foreach (var file in Directory.GetFiles(_sessionsDirectory, "*.gaprof"))
                {
                    var fi = new FileInfo(file);
                    sessions.Add(new SessionListItem
                    {
                        fileName = fi.Name,
                        sizeBytes = fi.Length,
                        created = fi.CreationTimeUtc.ToString("o")
                    });
                }
            }

            string json = "[" + string.Join(",", sessions.ConvertAll(s => JsonUtility.ToJson(s))) + "]";
            WriteJson(stream, 200, json);
        }

        private void HandleDownloadSession(NetworkStream stream, string path)
        {
            string[] parts = path.Split('/');
            if (parts.Length < 3)
            {
                WriteJson(stream, 400, "{\"error\":\"Invalid path\"}");
                return;
            }

            string fileName = Uri.UnescapeDataString(parts[2]);
            if (fileName.Contains("..") || fileName.Contains("/") || fileName.Contains("\\"))
            {
                WriteJson(stream, 400, "{\"error\":\"Invalid filename\"}");
                return;
            }

            string filePath = Path.Combine(_sessionsDirectory, fileName);
            if (!File.Exists(filePath))
            {
                WriteJson(stream, 404, "{\"error\":\"Session not found\"}");
                return;
            }

            using (var fs = File.OpenRead(filePath))
            {
                string headers =
                    "HTTP/1.1 200 OK\r\n" +
                    "Content-Type: application/octet-stream\r\n" +
                    $"Content-Length: {fs.Length}\r\n" +
                    $"Content-Disposition: attachment; filename=\"{fileName}\"\r\n" +
                    "Access-Control-Allow-Origin: *\r\n" +
                    "Connection: close\r\n\r\n";
                byte[] headerBytes = Encoding.UTF8.GetBytes(headers);
                stream.Write(headerBytes, 0, headerBytes.Length);
                fs.CopyTo(stream);
                stream.Flush();
            }
        }

        private void HandleStartCapture(NetworkStream stream, string body)
        {
            if (_profiler.State == CaptureState.Capturing)
            {
                WriteJson(stream, 409, "{\"error\":\"Already capturing\"}");
                return;
            }

            string sessionName = null;
            if (!string.IsNullOrEmpty(body) && body.Contains("\"name\""))
            {
                int start = body.IndexOf("\"name\"", StringComparison.Ordinal) + 6;
                int colon = body.IndexOf(':', start);
                int quote1 = body.IndexOf('"', colon + 1);
                int quote2 = quote1 >= 0 ? body.IndexOf('"', quote1 + 1) : -1;
                if (quote1 >= 0 && quote2 > quote1)
                {
                    sessionName = body.Substring(quote1 + 1, quote2 - quote1 - 1);
                }
            }

            MainThreadDispatcher.Enqueue(() => _profiler.StartCapture(sessionName));
            WriteJson(stream, 200, "{\"status\":\"started\"}");
        }

        private void HandleStopCapture(NetworkStream stream)
        {
            if (_profiler.State != CaptureState.Capturing)
            {
                WriteJson(stream, 409, "{\"error\":\"Not capturing\"}");
                return;
            }

            var done = new ManualResetEventSlim(false);
            SessionExportInfo exportInfo = default;

            void OnStopped(SessionExportInfo info)
            {
                exportInfo = info;
                done.Set();
            }

            try
            {
                _profiler.OnCaptureStopped += OnStopped;
                MainThreadDispatcher.Enqueue(() => _profiler.StopCapture());

                if (!done.Wait(TimeSpan.FromSeconds(300)))
                {
                    WriteJson(stream, 504, "{\"error\":\"Export timed out\"}");
                    return;
                }

                string filePath = exportInfo.filePath ?? string.Empty;
                string sessionName = exportInfo.sessionName ?? string.Empty;
                string json =
                    "{\"status\":\"stopped\"," +
                    $"\"filePath\":\"{EscapeJson(filePath)}\"," +
                    $"\"sessionName\":\"{EscapeJson(sessionName)}\"," +
                    $"\"frameCount\":{exportInfo.frameCount}," +
                    $"\"duration\":{exportInfo.duration.ToString(CultureInfo.InvariantCulture)}," +
                    $"\"screenshotCount\":{exportInfo.screenshotCount}," +
                    $"\"isDeepProfile\":{(_profiler.HasDeepProfileData ? "true" : "false")}," +
                    $"\"deepDataSize\":{_profiler.DeepProfileDataSize}" +
                    "}";
                WriteJson(stream, 200, json);
            }
            finally
            {
                _profiler.OnCaptureStopped -= OnStopped;
                done.Dispose();
            }
        }

        private void HandleDeepProfilingToggle(NetworkStream stream, string body)
        {
            if (_profiler.State == CaptureState.Capturing)
            {
                WriteJson(stream, 409, "{\"error\":\"Cannot toggle deep profiling while capturing\"}");
                return;
            }

            bool enabled = false;
            if (!string.IsNullOrEmpty(body) && body.Contains("\"enabled\""))
            {
                enabled = body.Contains("true");
            }

            using (var done = new ManualResetEvent(false))
            {
                bool applied = false;
                MainThreadDispatcher.Enqueue(() =>
                {
                    try
                    {
                        applied = _profiler.SetDeepCaptureEnabled(enabled);
                    }
                    finally
                    {
                        done.Set();
                    }
                });

                if (!done.WaitOne(1000))
                {
                    WriteJson(stream, 504, "{\"error\":\"Toggle deep profiling timed out\"}");
                    return;
                }

                if (!applied)
                {
                    WriteJson(stream, 500, "{\"error\":\"Failed to apply deep profiling state\"}");
                    return;
                }
            }

            bool actual = _profiler.config != null && _profiler.config.enableDeepCapture;
            WriteJson(stream, 200, $"{{\"status\":\"ok\",\"deepCapture\":{(actual ? "true" : "false")}}}");
        }

        private void HandleDeepDataDownload(NetworkStream stream, string path)
        {
            // path: /sessions/{name}/deep-download
            string[] parts = path.Split('/');
            if (parts.Length < 4)
            {
                WriteJson(stream, 400, "{\"error\":\"Invalid path\"}");
                return;
            }

            string sessionFileBaseName = Uri.UnescapeDataString(parts[2]);
            string dataFilePath = _profiler.GetDeepProfileDataPathForSession(sessionFileBaseName);
            if (string.IsNullOrEmpty(dataFilePath) || !File.Exists(dataFilePath))
            {
                WriteJson(stream, 404, "{\"error\":\"No deep profile data available\"}");
                return;
            }

#if UNITY_EDITOR
            string exportedPath;
            string exportError;
            if (TryExportDeepDataForCurrentEditor(dataFilePath, out exportedPath, out exportError))
            {
                if (!string.IsNullOrEmpty(exportedPath) && File.Exists(exportedPath))
                {
                    dataFilePath = exportedPath;
                }
            }
            else if (!string.IsNullOrEmpty(exportError))
            {
                WriteJson(stream, 500, $"{{\"error\":\"{EscapeJson(exportError)}\"}}");
                return;
            }
#endif

            using (var fs = File.OpenRead(dataFilePath))
            {
                string fileName = Path.GetFileName(dataFilePath);
                string headers =
                    "HTTP/1.1 200 OK\r\n" +
                    "Content-Type: application/octet-stream\r\n" +
                    $"Content-Length: {fs.Length}\r\n" +
                    $"Content-Disposition: attachment; filename=\"{fileName}\"\r\n" +
                    "Access-Control-Allow-Origin: *\r\n" +
                    "Connection: close\r\n\r\n";
                byte[] headerBytes = Encoding.UTF8.GetBytes(headers);
                stream.Write(headerBytes, 0, headerBytes.Length);

                // Stream in chunks for large files
                byte[] buffer = new byte[65536]; // 64KB chunks
                int read;
                while ((read = fs.Read(buffer, 0, buffer.Length)) > 0)
                {
                    stream.Write(buffer, 0, read);
                }
                stream.Flush();
            }
        }

#if UNITY_EDITOR
        private static bool TryExportDeepDataForCurrentEditor(string rawPath, out string exportPath, out string error)
        {
            string resultPath = null;
            string resultError = null;

            using (var done = new ManualResetEvent(false))
            {
                MainThreadDispatcher.Enqueue(() =>
                {
                    try
                    {
                        var exporterType = Type.GetType("GameAnalytics.Profiler.Editor.DeepProfileExporter, GameAnalytics.Profiler.Editor");
                        if (exporterType == null)
                        {
                            resultError = "DeepProfileExporter type not found in editor assembly";
                            return;
                        }

                        var method = exporterType.GetMethod("ExportForRuntime", BindingFlags.Public | BindingFlags.Static);
                        if (method == null)
                        {
                            resultError = "DeepProfileExporter.ExportForRuntime not found";
                            return;
                        }

                        resultPath = method.Invoke(null, new object[] { rawPath }) as string;
                    }
                    catch (TargetInvocationException tie)
                    {
                        resultError = tie.InnerException != null ? tie.InnerException.ToString() : tie.ToString();
                    }
                    catch (Exception ex)
                    {
                        resultError = ex.ToString();
                    }
                    finally
                    {
                        done.Set();
                    }
                });

                if (!done.WaitOne(30000))
                {
                    resultError = "Timed out while exporting deep profiler data in current Unity Editor";
                }
            }

            exportPath = resultPath;
            error = resultError;
            return string.IsNullOrEmpty(resultError);
        }
#endif

        private static void WriteJson(NetworkStream stream, int status, string json)
        {
            string headers =
                $"HTTP/1.1 {status} {GetReasonPhrase(status)}\r\n" +
                "Content-Type: application/json\r\n" +
                "Access-Control-Allow-Origin: *\r\n" +
                "Connection: close\r\n" +
                $"Content-Length: {Encoding.UTF8.GetByteCount(json)}\r\n\r\n";

            byte[] headerBytes = Encoding.UTF8.GetBytes(headers);
            byte[] bodyBytes = Encoding.UTF8.GetBytes(json);
            stream.Write(headerBytes, 0, headerBytes.Length);
            stream.Write(bodyBytes, 0, bodyBytes.Length);
            stream.Flush();
        }

        private static string GetReasonPhrase(int status)
        {
            switch (status)
            {
                case 200: return "OK";
                case 400: return "Bad Request";
                case 404: return "Not Found";
                case 409: return "Conflict";
                case 500: return "Internal Server Error";
                case 504: return "Gateway Timeout";
                default: return "OK";
            }
        }

        private static string EscapeJson(string s)
        {
            return (s ?? string.Empty).Replace("\\", "\\\\").Replace("\"", "\\\"").Replace("\n", "\\n");
        }

        private sealed class SseClient
        {
            public TcpClient Client;
            public StreamWriter Writer;
        }

        [Serializable]
        private class StatusResponse
        {
            public string deviceModel;
            public string projectName;
            public string sdkVersion;
            public bool capturing;
            public int frameCount;
            public float elapsed;
            public float currentFps;
            public bool deepCaptureEnabled;
            public bool hasDeepData;
            public long deepDataSize;
        }

        [Serializable]
        private class SessionListItem
        {
            public string fileName;
            public long sizeBytes;
            public string created;
        }
    }

    public class MainThreadDispatcher : MonoBehaviour
    {
        private static MainThreadDispatcher _instance;
        private static readonly Queue<Action> Queue = new Queue<Action>();

        public static void EnsureInitialized()
        {
            if (_instance != null) return;
            var go = new GameObject("GAProfiler_Dispatcher");
            _instance = go.AddComponent<MainThreadDispatcher>();
            DontDestroyOnLoad(go);
        }

        public static void Enqueue(Action action)
        {
            if (action == null) return;
            lock (Queue)
            {
                Queue.Enqueue(action);
            }

            if (_instance == null) return;
        }

        private void Update()
        {
            lock (Queue)
            {
                while (Queue.Count > 0)
                {
                    try
                    {
                        Queue.Dequeue()?.Invoke();
                    }
                    catch (Exception e)
                    {
                        Debug.LogError($"[GAProfiler] Dispatcher error: {e}");
                    }
                }
            }
        }

        private void OnDestroy()
        {
            if (_instance == this) _instance = null;
        }
    }
}

#endif
