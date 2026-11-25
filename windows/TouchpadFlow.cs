using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Management;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Threading;
using System.Windows.Forms;
using Microsoft.Win32;

namespace TouchpadFlow
{
    internal static class Program
    {
        private const string MutexName = "TouchpadFlow_SingleInstance_Mutex";
        internal static readonly string LogPath = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "TouchpadFlow", "TouchpadFlow.log");

        internal static void Log(string message)
        {
            try
            {
                string dir = Path.GetDirectoryName(LogPath);
                if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir))
                    Directory.CreateDirectory(dir);
                File.AppendAllText(LogPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss.fff}] {message}\r\n");
            }
            catch { }
        }

        [STAThread]
        private static int Main(string[] args)
        {
            using (var mutex = new Mutex(true, MutexName, out bool createdNew))
            {
                if (!createdNew)
                {
                    MessageBox.Show("TouchpadFlow is already running in the system tray.",
                        "TouchpadFlow", MessageBoxButtons.OK, MessageBoxIcon.Information);
                    return 0;
                }

                Application.EnableVisualStyles();
                Application.SetCompatibleTextRenderingDefault(false);

                try
                {
                    using (var app = new FlowApp(args.Contains("--diagnose")))
                    {
                        Application.Run(app);
                    }
                    return 0;
                }
                catch (Exception ex)
                {
                    Log(ex.ToString());
                    MessageBox.Show($"{ex.Message}\n\nLog: {LogPath}",
                        "TouchpadFlow Error", MessageBoxButtons.OK, MessageBoxIcon.Error);
                    return 1;
                }
            }
        }
    }

    internal sealed class ContactPoint
    {
        internal int Id;
        internal double X, Y;
        internal bool Tip;
        internal bool Confident;
    }

    internal sealed class MultiFrame
    {
        internal int ContactCount;
        internal bool Click;
        internal readonly List<ContactPoint> Contacts = new List<ContactPoint>();
    }

    /// <summary>Shared settings file written by touchpadflow-app (same schema on every OS).</summary>
    internal sealed class SharedConfig
    {
        internal static readonly string Path = System.IO.Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "TouchpadFlow", "touchpadflow.json");

        internal bool Enabled = true;
        internal double LeftRatio = 0.22, RightRatio = 0.22;
        internal string LeftAction = "brightness", RightAction = "volume";
        internal double LeftSensitivity = 1.5, RightSensitivity = 1.5;

        internal static SharedConfig Load()
        {
            var c = new SharedConfig();
            try
            {
                if (!File.Exists(Path)) return c;
                using var doc = JsonDocument.Parse(File.ReadAllText(Path));
                var r = doc.RootElement;
                if (r.TryGetProperty("enabled", out var v) && (v.ValueKind == JsonValueKind.True || v.ValueKind == JsonValueKind.False)) c.Enabled = v.GetBoolean();
                c.LeftRatio = Num(r, "left_edge_ratio", c.LeftRatio, 0.05, 0.45);
                c.RightRatio = Num(r, "right_edge_ratio", c.RightRatio, 0.05, 0.45);
                c.LeftSensitivity = Num(r, "left_sensitivity", c.LeftSensitivity, 0.2, 4.0);
                c.RightSensitivity = Num(r, "right_sensitivity", c.RightSensitivity, 0.2, 4.0);
                if (r.TryGetProperty("left_action", out v) && v.ValueKind == JsonValueKind.String) c.LeftAction = v.GetString() ?? c.LeftAction;
                if (r.TryGetProperty("right_action", out v) && v.ValueKind == JsonValueKind.String) c.RightAction = v.GetString() ?? c.RightAction;
            }
            catch (Exception ex)
            {
                Program.Log($"Config {Path} unreadable, using defaults: {ex.Message}");
            }
            return c;
        }

        private static double Num(JsonElement r, string name, double fallback, double min, double max)
        {
            if (r.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number)
                return Math.Max(min, Math.Min(max, v.GetDouble()));
            return fallback;
        }
    }

    internal sealed class FlowGesture
    {
        internal SharedConfig Config = SharedConfig.Load();
        internal bool RequireTwoFingers = true;

        private bool armed;
        private bool active;
        private int side; // -1 = left zone, 1 = right zone (action comes from Config)
        private double previousAvgY;
        private double startAvgY;
        private long lastTimestamp;

        internal int Side => side;
        internal bool Active => active;

        internal void Reset()
        {
            armed = false;
            active = false;
            side = 0;
            lastTimestamp = 0;
        }

        internal double Feed(MultiFrame frame, long now)
        {
            if (lastTimestamp != 0 && (now - lastTimestamp) > 350)
                Reset();
            lastTimestamp = now;

            int targetFingers = RequireTwoFingers ? 2 : 1;

            if (frame.ContactCount == 0 || frame.Click)
            {
                Reset();
                return 0;
            }

            // Must match target finger count exactly
            if (frame.ContactCount != targetFingers || frame.Contacts.Count < targetFingers)
            {
                if (active) Reset();
                return 0;
            }

            // Ensure contacts have tip contact and palm rejection confidence
            if (!frame.Contacts.Take(targetFingers).All(c => c.Tip && c.Confident))
            {
                if (active) Reset();
                return 0;
            }

            double avgX = frame.Contacts.Take(targetFingers).Average(c => c.X);
            double avgY = frame.Contacts.Take(targetFingers).Average(c => c.Y);

            if (!armed)
            {
                // Must initiate strictly inside an enabled edge zone
                if (avgX <= Config.LeftRatio && Config.LeftAction != "none")
                    side = -1;
                else if (avgX >= (1.0 - Config.RightRatio) && Config.RightAction != "none")
                    side = 1;
                else
                {
                    Reset();
                    return 0;
                }

                armed = true;
                startAvgY = previousAvgY = avgY;
                return 0;
            }

            // Check if fingers wandered too far into the center
            if (side < 0 && avgX > (Config.LeftRatio + 0.08))
            {
                Reset();
                return 0;
            }
            if (side > 0 && avgX < (1.0 - Config.RightRatio - 0.08))
            {
                Reset();
                return 0;
            }

            if (!active)
            {
                double dy = Math.Abs(avgY - startAvgY);
                if (dy < 0.02) return 0; // Arming deadband
                active = true;
                previousAvgY = avgY;
                return 0;
            }

            double sensitivity = side < 0 ? Config.LeftSensitivity : Config.RightSensitivity;
            double delta = (previousAvgY - avgY) * 100.0 * sensitivity;
            previousAvgY = avgY;
            return delta;
        }
    }

    internal static class Native
    {
        internal const uint Success = 0x00110000;

        [StructLayout(LayoutKind.Sequential)]
        internal struct RawDevice
        {
            internal ushort Page;
            internal ushort Usage;
            internal uint Flags;
            internal IntPtr Target;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Header
        {
            internal uint Type;
            internal uint Size;
            internal IntPtr Device;
            internal IntPtr WParam;
        }

        [StructLayout(LayoutKind.Sequential)]
        internal struct Caps
        {
            internal ushort Usage, Page, InputLength, OutputLength, FeatureLength;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 17)]
            internal ushort[] Reserved;
            internal ushort Nodes, InputButtons, InputValues, InputIndices,
                OutputButtons, OutputValues, OutputIndices, FeatureButtons, FeatureValues, FeatureIndices;
        }

        [StructLayout(LayoutKind.Explicit, Size = 72)]
        internal struct ValueCaps
        {
            [FieldOffset(0)] internal ushort Page;
            [FieldOffset(2)] internal byte ReportId;
            [FieldOffset(6)] internal ushort Link;
            [FieldOffset(12)] internal byte IsRange;
            [FieldOffset(18)] internal ushort BitSize;
            [FieldOffset(40)] internal int Min;
            [FieldOffset(44)] internal int Max;
            [FieldOffset(56)] internal ushort Usage;
            [FieldOffset(58)] internal ushort UsageMax;

            internal bool Has(ushort page, ushort usage) =>
                Page == page && (IsRange == 0 ? Usage == usage : Usage <= usage && usage <= UsageMax);
        }

        [DllImport("user32.dll", SetLastError = true)]
        internal static extern bool RegisterRawInputDevices(RawDevice[] devices, uint count, uint size);

        [DllImport("user32.dll", SetLastError = true)]
        internal static extern uint GetRawInputData(IntPtr input, uint command, IntPtr data, ref uint size, uint headerSize);

        [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        internal static extern uint GetRawInputDeviceInfo(IntPtr device, uint command, IntPtr data, ref uint size);

        [DllImport("hid.dll")]
        internal static extern uint HidP_GetCaps(IntPtr data, out Caps caps);

        [DllImport("hid.dll")]
        internal static extern uint HidP_GetValueCaps(int type, [Out] ValueCaps[] caps, ref ushort count, IntPtr data);

        [DllImport("hid.dll")]
        internal static extern uint HidP_GetUsageValue(int type, ushort page, ushort link, ushort usage, out uint value, IntPtr data, byte[] report, uint length);

        [DllImport("hid.dll")]
        internal static extern uint HidP_GetUsages(int type, ushort page, ushort link, [Out] ushort[] usages, ref uint count, IntPtr data, byte[] report, uint length);
    }

    internal sealed class TouchpadParser : IDisposable
    {
        private struct Slot
        {
            internal Native.ValueCaps X, Y, ContactId;
        }

        private readonly IntPtr preparsedData;
        private readonly Slot[] slots;
        private readonly Native.ValueCaps[] countCaps;
        private readonly ushort[] buttonUsages = new ushort[32];

        internal TouchpadParser(IntPtr rawDevice)
        {
            uint size = 0;
            Native.GetRawInputDeviceInfo(rawDevice, 0x20000005, IntPtr.Zero, ref size);
            if (size == 0) throw new InvalidOperationException("Cannot query HID preparsed data size");

            preparsedData = Marshal.AllocHGlobal((int)size);
            if (Native.GetRawInputDeviceInfo(rawDevice, 0x20000005, preparsedData, ref size) == uint.MaxValue)
                throw new InvalidOperationException("Cannot read HID preparsed data");

            if (Native.HidP_GetCaps(preparsedData, out Native.Caps caps) != Native.Success)
                throw new InvalidOperationException("Cannot read HID device capabilities");

            ushort numValues = caps.InputValues;
            var values = new Native.ValueCaps[numValues];
            if (Native.HidP_GetValueCaps(0, values, ref numValues, preparsedData) != Native.Success)
                throw new InvalidOperationException("Cannot read HID value capabilities");

            var xs = values.Where(v => v.Has(1, 0x30)).ToArray();
            var ys = values.Where(v => v.Has(1, 0x31)).ToArray();
            var ids = values.Where(v => v.Has(13, 0x51)).ToArray();

            countCaps = values.Where(v => v.Has(13, 0x54)).ToArray();

            var list = new List<Slot>();
            foreach (var x in xs)
            {
                var y = ys.FirstOrDefault(v => v.ReportId == x.ReportId && v.Link == x.Link);
                if (y.Page == 0) continue;
                var cid = ids.FirstOrDefault(v => v.ReportId == x.ReportId && v.Link == x.Link);
                list.Add(new Slot { X = x, Y = y, ContactId = cid });
            }
            slots = list.ToArray();
        }

        internal MultiFrame Parse(byte[] report)
        {
            if (slots.Length == 0) return null;
            byte reportId = report[0];

            int totalContacts = 0;
            var matchingCounts = countCaps.Where(c => c.ReportId == reportId);
            foreach (var c in matchingCounts)
            {
                if (Native.HidP_GetUsageValue(0, c.Page, c.Link, 0x54, out uint cnt, preparsedData, report, (uint)report.Length) == Native.Success)
                {
                    totalContacts = (int)cnt;
                    break;
                }
            }

            var frame = new MultiFrame();
            var activeSlots = slots.Where(s => s.X.ReportId == reportId);

            foreach (var s in activeSlots)
            {
                uint numButtons = (uint)buttonUsages.Length;
                bool tip = false;
                bool confident = true;

                if (Native.HidP_GetUsages(0, 13, s.X.Link, buttonUsages, ref numButtons, preparsedData, report, (uint)report.Length) == Native.Success)
                {
                    for (int i = 0; i < numButtons; i++)
                    {
                        if (buttonUsages[i] == 0x42) tip = true;
                        if (buttonUsages[i] == 0x47) confident = true;
                    }
                }

                if (!tip) continue;

                if (Native.HidP_GetUsageValue(0, 1, s.X.Link, 0x30, out uint rawX, preparsedData, report, (uint)report.Length) == Native.Success &&
                    Native.HidP_GetUsageValue(0, 1, s.Y.Link, 0x31, out uint rawY, preparsedData, report, (uint)report.Length) == Native.Success)
                {
                    double normX = (double)(rawX - s.X.Min) / (s.X.Max - s.X.Min);
                    double normY = (double)(rawY - s.Y.Min) / (s.Y.Max - s.Y.Min);

                    uint contactId = 0;
                    if (s.ContactId.Page != 0)
                        Native.HidP_GetUsageValue(0, 13, s.ContactId.Link, 0x51, out contactId, preparsedData, report, (uint)report.Length);

                    frame.Contacts.Add(new ContactPoint
                    {
                        Id = (int)contactId,
                        X = Math.Max(0.0, Math.Min(1.0, normX)),
                        Y = Math.Max(0.0, Math.Min(1.0, normY)),
                        Tip = tip,
                        Confident = confident
                    });
                }
            }

            frame.ContactCount = Math.Max(totalContacts, frame.Contacts.Count);
            return frame;
        }

        public void Dispose()
        {
            if (preparsedData != IntPtr.Zero)
                Marshal.FreeHGlobal(preparsedData);
        }
    }

    [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")]
    internal class MMDeviceEnumerator { }

    [ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    internal interface IMMDeviceEnumerator
    {
        [PreserveSig] int EnumAudioEndpoints(int flow, uint mask, out IntPtr devices);
        [PreserveSig] int GetDefaultAudioEndpoint(int flow, int role, out IMMDevice device);
    }

    [ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    internal interface IMMDevice
    {
        [PreserveSig] int Activate(ref Guid iid, uint context, IntPtr parameters, [MarshalAs(UnmanagedType.IUnknown)] out object instance);
    }

    [ComImport, Guid("5CDF2C82-841E-4546-9722-0CF74078229A"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    internal interface IAudioEndpointVolume
    {
        [PreserveSig] int RegisterControlChangeNotify(IntPtr notify);
        [PreserveSig] int UnregisterControlChangeNotify(IntPtr notify);
        [PreserveSig] int GetChannelCount(out uint count);
        [PreserveSig] int SetMasterVolumeLevel(float level, ref Guid context);
        [PreserveSig] int SetMasterVolumeLevelScalar(float level, ref Guid context);
        [PreserveSig] int GetMasterVolumeLevel(out float level);
        [PreserveSig] int GetMasterVolumeLevelScalar(out float level);
        [PreserveSig] int SetChannelVolumeLevel(uint channel, float level, ref Guid context);
        [PreserveSig] int SetChannelVolumeLevelScalar(uint channel, float level, ref Guid context);
        [PreserveSig] int GetChannelVolumeLevel(uint channel, out float level);
        [PreserveSig] int GetChannelVolumeLevelScalar(uint channel, out float level);
        [PreserveSig] int SetMute([MarshalAs(UnmanagedType.Bool)] bool mute, ref Guid context);
        [PreserveSig] int GetMute([MarshalAs(UnmanagedType.Bool)] out bool mute);
    }

    internal static class WindowsControls
    {
        internal static double AdjustBrightness(double? target)
        {
            string className = target.HasValue ? "WmiMonitorBrightnessMethods" : "WmiMonitorBrightness";
            using (var search = new ManagementObjectSearcher("root\\WMI", $"SELECT * FROM {className} WHERE Active = TRUE"))
            using (var results = search.Get())
            {
                foreach (ManagementObject monitor in results)
                using (monitor)
                {
                    if (!target.HasValue)
                        return Convert.ToDouble(monitor["CurrentBrightness"]);

                    byte val = (byte)Math.Max(1, Math.Min(100, Math.Round(target.Value)));
                    monitor.InvokeMethod("WmiSetBrightness", new object[] { (uint)0, val });
                    return val;
                }
            }
            throw new InvalidOperationException("Built-in display brightness is unavailable via WMI");
        }

        internal static double AdjustVolume(double? target)
        {
            var enumerator = (IMMDeviceEnumerator)new MMDeviceEnumerator();
            IMMDevice device = null;
            object endpoint = null;
            try
            {
                Marshal.ThrowExceptionForHR(enumerator.GetDefaultAudioEndpoint(0, 1, out device));
                Guid iid = typeof(IAudioEndpointVolume).GUID;
                Marshal.ThrowExceptionForHR(device.Activate(ref iid, 23, IntPtr.Zero, out endpoint));
                var volume = (IAudioEndpointVolume)endpoint;

                Guid context = Guid.Empty;
                if (target.HasValue)
                {
                    float scalar = (float)(Math.Max(0, Math.Min(100, target.Value)) / 100.0);
                    Marshal.ThrowExceptionForHR(volume.SetMasterVolumeLevelScalar(scalar, ref context));
                    if (scalar > 0)
                        Marshal.ThrowExceptionForHR(volume.SetMute(false, ref context));
                }

                Marshal.ThrowExceptionForHR(volume.GetMasterVolumeLevelScalar(out float current));
                return current * 100.0;
            }
            finally
            {
                if (endpoint != null) Marshal.ReleaseComObject(endpoint);
                if (device != null) Marshal.ReleaseComObject(device);
                Marshal.ReleaseComObject(enumerator);
            }
        }
    }

    internal sealed class WindowsOsd : IDisposable
    {
        [ComImport, Guid("6D5140C1-7436-11CE-8034-00AA006009FA"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        private interface IShellServiceProvider
        {
            [PreserveSig] int QueryService(ref Guid service, ref Guid iid, out IntPtr result);
        }

        [DllImport("ole32.dll", ExactSpelling = true)]
        private static extern int CoCreateInstance(ref Guid clsid, IntPtr outer, uint context,
            ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out IShellServiceProvider provider);

        [UnmanagedFunctionPointer(CallingConvention.StdCall)]
        private delegate int ShowFlyout(IntPtr self, int kind, uint command);

        private IntPtr flyout;
        private ShowFlyout show;

        internal void Display(int side)
        {
            try
            {
                if (flyout == IntPtr.Zero)
                {
                    Guid clsid = new Guid("C2F03A33-21F5-47FA-B4BB-156362A2F239");
                    Guid iid = typeof(IShellServiceProvider).GUID;
                    IShellServiceProvider provider = null;
                    try
                    {
                        Marshal.ThrowExceptionForHR(CoCreateInstance(ref clsid, IntPtr.Zero, 4, ref iid, out provider));
                        Guid service = new Guid("41F9D2FB-7834-4AB6-8B1B-73E74064B465");
                        iid = service;
                        Marshal.ThrowExceptionForHR(provider.QueryService(ref service, ref iid, out flyout));
                        show = Marshal.GetDelegateForFunctionPointer<ShowFlyout>(
                            Marshal.ReadIntPtr(Marshal.ReadIntPtr(flyout), 3 * IntPtr.Size));
                    }
                    finally
                    {
                        if (provider != null) Marshal.ReleaseComObject(provider);
                    }
                }
                // side < 0 is brightness (kind 3), side > 0 is volume (kind 0)
                Marshal.ThrowExceptionForHR(show(flyout, side < 0 ? 3 : 0, 0));
            }
            catch { }
        }

        public void Dispose()
        {
            if (flyout != IntPtr.Zero)
            {
                Marshal.Release(flyout);
                flyout = IntPtr.Zero;
            }
            show = null;
        }
    }

    internal sealed class FlowApp : Form
    {
        private const int WM_INPUT = 0x00FF;
        private readonly NotifyIcon trayIcon;
        private readonly FlowGesture gesture = new FlowGesture();
        private readonly WindowsOsd osd = new WindowsOsd();
        private TouchpadParser parser;
        private bool isPaused;
        private DateTime configStamp = DateTime.MinValue;
        private double? targetVolume, targetBrightness;
        private readonly object lockObj = new object();
        private readonly AutoResetEvent workerWake = new AutoResetEvent(false);
        private readonly Thread workerThread;
        private bool workerRunning = true;

        internal FlowApp(bool diagnose)
        {
            ShowInTaskbar = false;
            WindowState = FormWindowState.Minimized;
            FormBorderStyle = FormBorderStyle.None;
            Size = new Size(0, 0);

            trayIcon = new NotifyIcon
            {
                Icon = SystemIcons.Application,
                Text = "TouchpadFlow (Edge Volume & Brightness)",
                Visible = true
            };

            var contextMenu = new ContextMenuStrip();
            var modeItem = new ToolStripMenuItem("Mode: Two Fingers (Default)", null, (s, e) =>
            {
                gesture.RequireTwoFingers = !gesture.RequireTwoFingers;
                ((ToolStripMenuItem)s).Text = gesture.RequireTwoFingers
                    ? "Mode: Two Fingers (Default)"
                    : "Mode: One Finger (Classic)";
            });

            var pauseItem = new ToolStripMenuItem("Pause", null, (s, e) =>
            {
                isPaused = !isPaused;
                ((ToolStripMenuItem)s).Text = isPaused ? "Resume" : "Pause";
                trayIcon.Text = isPaused ? "TouchpadFlow (Paused)" : "TouchpadFlow (Active)";
            });

            var startWithWindowsItem = new ToolStripMenuItem("Start with Windows", null, ToggleStartup);
            startWithWindowsItem.Checked = IsStartupEnabled();

            var exitItem = new ToolStripMenuItem("Exit", null, (s, e) => Application.Exit());

            contextMenu.Items.Add(modeItem);
            contextMenu.Items.Add(new ToolStripSeparator());
            contextMenu.Items.Add(pauseItem);
            contextMenu.Items.Add(startWithWindowsItem);
            contextMenu.Items.Add(new ToolStripSeparator());
            contextMenu.Items.Add(exitItem);

            trayIcon.ContextMenuStrip = contextMenu;

            // Pick up changes saved by touchpadflow-app without a restart.
            var configTimer = new System.Windows.Forms.Timer { Interval = 1000 };
            configTimer.Tick += (s, e) =>
            {
                DateTime stamp = File.Exists(SharedConfig.Path) ? File.GetLastWriteTimeUtc(SharedConfig.Path) : DateTime.MinValue;
                if (stamp == configStamp) return;
                configStamp = stamp;
                gesture.Config = SharedConfig.Load();
                gesture.Reset();
            };
            configTimer.Start();

            RegisterRawInput();

            workerThread = new Thread(WorkerLoop)
            {
                IsBackground = true,
                Name = "TouchpadFlow Output Worker"
            };
            workerThread.Start();
        }

        private void RegisterRawInput()
        {
            var dev = new Native.RawDevice
            {
                Page = 0x0D,   // Digitizer
                Usage = 0x05,  // Touch Pad
                Flags = 0x00000100, // RIDEV_INPUTSINK
                Target = Handle
            };

            if (!Native.RegisterRawInputDevices(new[] { dev }, 1, (uint)Marshal.SizeOf(typeof(Native.RawDevice))))
                throw new InvalidOperationException("Failed to register Windows Precision Touchpad Raw Input.");
        }

        protected override void WndProc(ref Message m)
        {
            if (m.Msg == WM_INPUT && !isPaused && gesture.Config.Enabled)
            {
                ProcessRawInput(m.LParam);
            }
            base.WndProc(ref m);
        }

        private void ProcessRawInput(IntPtr hRawInput)
        {
            uint size = 0;
            Native.GetRawInputData(hRawInput, 0x10000003, IntPtr.Zero, ref size, (uint)Marshal.SizeOf(typeof(Native.Header)));
            if (size == 0) return;

            IntPtr buffer = Marshal.AllocHGlobal((int)size);
            try
            {
                if (Native.GetRawInputData(hRawInput, 0x10000003, buffer, ref size, (uint)Marshal.SizeOf(typeof(Native.Header))) == uint.MaxValue)
                    return;

                var header = Marshal.PtrToStructure<Native.Header>(buffer);
                if (parser == null)
                {
                    try { parser = new TouchpadParser(header.Device); }
                    catch { return; }
                }

                int rawReportSize = (int)(size - Marshal.SizeOf(typeof(Native.Header)) - 8);
                if (rawReportSize <= 0) return;

                byte[] report = new byte[rawReportSize];
                Marshal.Copy(IntPtr.Add(buffer, Marshal.SizeOf(typeof(Native.Header)) + 8), report, 0, rawReportSize);

                MultiFrame frame = parser.Parse(report);
                if (frame != null)
                {
                    long now = Stopwatch.GetTimestamp() / (Stopwatch.Frequency / 1000);
                    double delta = gesture.Feed(frame, now);

                    if (gesture.Active && Math.Abs(delta) > 0.001)
                    {
                        ApplyDelta(gesture.Side, delta);
                    }
                }
            }
            finally
            {
                Marshal.FreeHGlobal(buffer);
            }
        }

        private void ApplyDelta(int side, double delta)
        {
            string action = side < 0 ? gesture.Config.LeftAction : gesture.Config.RightAction;
            if (action == "none") return;
            int target = action == "brightness" ? -1 : 1; // OSD/worker use -1 = brightness, 1 = volume
            lock (lockObj)
            {
                if (target < 0)
                {
                    if (!targetBrightness.HasValue)
                        targetBrightness = WindowsControls.AdjustBrightness(null);
                    targetBrightness = Math.Max(1.0, Math.Min(100.0, targetBrightness.Value + delta));
                }
                else
                {
                    if (!targetVolume.HasValue)
                        targetVolume = WindowsControls.AdjustVolume(null);
                    targetVolume = Math.Max(0.0, Math.Min(100.0, targetVolume.Value + delta));
                }
            }
            workerWake.Set();
            osd.Display(target);
        }

        private void WorkerLoop()
        {
            while (workerRunning)
            {
                workerWake.WaitOne();
                Thread.Sleep(25);

                double? b = null, v = null;
                lock (lockObj)
                {
                    b = targetBrightness;
                    v = targetVolume;
                    targetBrightness = null;
                    targetVolume = null;
                }

                if (b.HasValue)
                {
                    try { WindowsControls.AdjustBrightness(b.Value); } catch { }
                }
                if (v.HasValue)
                {
                    try { WindowsControls.AdjustVolume(v.Value); } catch { }
                }
            }
        }

        private bool IsStartupEnabled()
        {
            using (var key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Run", false))
                return key?.GetValue("TouchpadFlow") != null;
        }

        private void ToggleStartup(object sender, EventArgs e)
        {
            var item = (ToolStripMenuItem)sender;
            using (var key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Run", true))
            {
                if (item.Checked)
                {
                    key?.DeleteValue("TouchpadFlow", false);
                    item.Checked = false;
                }
                else
                {
                    key?.SetValue("TouchpadFlow", $"\"{Application.ExecutablePath}\"");
                    item.Checked = true;
                }
            }
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                workerRunning = false;
                workerWake.Set();
                trayIcon?.Dispose();
                parser?.Dispose();
                osd?.Dispose();
            }
            base.Dispose(disposing);
        }
    }
}
