using System.Runtime.InteropServices;
using System.Threading.Channels;

namespace OpenGHub.Core.Hidpp;

/// <summary>
/// A HID++ error response: the device rejected a call instead of returning
/// a normal result. <see cref="Function"/> is in pre-shifted form (0x00, 0x10, ...).
/// </summary>
public sealed class HidppErrorException : Exception
{
    public HidppErrorException(byte featureIndex, byte function, byte code)
        : base($"hidpp: device error 0x{code:x2} (feature index 0x{featureIndex:x2}, function 0x{function:x2})")
    {
        FeatureIndex = featureIndex;
        Function = function;
        Code = code;
    }

    public byte FeatureIndex { get; }
    public byte Function { get; }
    public byte Code { get; }
}

/// <summary>
/// An unsolicited HID++ report: one that didn't match any pending call.
/// </summary>
public sealed record Notification(byte DeviceIndex, byte[] Data);

/// <summary>
/// Transport layer of Logitech's HID++ 2.0 protocol over a single hidraw stream.
/// Ported from internal/hidpp/hidpp.go; see that file's package comment for the
/// wire format. One Conn can carry traffic for several device indices at once,
/// as when several peripherals are paired to the same wireless receiver.
/// </summary>
public sealed class HidppConn : IAsyncDisposable
{
    private const byte ReportIdShort = 0x10;
    private const byte ReportIdLong = 0x11;
    private const int LongReportLen = 20;

    /// <summary>
    /// Addresses a device connected directly (wired USB or Bluetooth),
    /// as opposed to one paired through a receiver.
    /// </summary>
    public const byte DeviceIndexDirect = 0xff;

    /// <summary>
    /// The Root feature is always at feature index 0 per the spec.
    /// </summary>
    public const byte RootFeatureIndex = 0x00;

    private readonly Stream _stream;
    private readonly int _fd;
    private readonly CancellationTokenSource _cts = new();
    private readonly Channel<Notification> _notifications;
    private Task? _readTask;
    private bool _readerStarted;

    private readonly object _gate = new();
    private TimeSpan _timeout;
    private byte _nextSw = 1;
    private readonly Dictionary<CallKey, TaskCompletionSource<CallResult>> _pending = new();
    private bool _closed;

    private readonly record struct CallKey(byte DeviceIndex, byte FeatureIndex, byte FuncSwId);
    private readonly record struct CallResult(byte[] Data, HidppErrorException? Error);

    [StructLayout(LayoutKind.Sequential)]
    private readonly struct PollFd
    {
        public readonly int Fd;
        public readonly short Events;
        public readonly short Revents;

        public PollFd(int fd, short events)
        {
            Fd = fd;
            Events = events;
            Revents = 0;
        }
    }

    [DllImport("libc", SetLastError = true)]
    private static extern int poll(ref PollFd fds, nuint nfds, int timeout);

    private const short PollIn = 0x0001;
    private const int Eintr = 4;

    public HidppConn(Stream stream)
    {
        _stream = stream;
        _fd = (int)((FileStream)stream).SafeFileHandle.DangerousGetHandle();
        _timeout = TimeSpan.FromSeconds(2);
        _notifications = Channel.CreateBounded<Notification>(
            new BoundedChannelOptions(16) { FullMode = BoundedChannelFullMode.DropWrite });
    }

    /// <summary>
    /// Reader for unsolicited reports; never blocks the read loop —
    /// reports are dropped rather than stalling response delivery.
    /// </summary>
    public ChannelReader<Notification> Notifications => _notifications.Reader;

    /// <summary>
    /// Sets how long subsequent calls wait for a response. Useful to
    /// shorten it while probing several device indices behind a receiver.
    /// </summary>
    public void SetTimeout(TimeSpan timeout)
    {
        lock (_gate) _timeout = timeout;
    }

    /// <summary>
    /// Invokes <paramref name="function"/> (pre-shifted into the high nibble,
    /// e.g. 0x10 for function 1) on <paramref name="featureIndex"/> for
    /// <paramref name="deviceIndex"/>, returning the response's parameter bytes.
    /// </summary>
    public async Task<byte[]> CallAsync(
        byte deviceIndex, byte featureIndex, byte function,
        byte[] parameters, CancellationToken ct = default)
    {
        if (parameters.Length > LongReportLen - 4)
            throw new ArgumentException($"too many parameters ({parameters.Length} > {LongReportLen - 4})", nameof(parameters));

        byte swId = AllocSoftwareId();
        byte funcSwId = (byte)((function & 0xf0) | swId);
        var key = new CallKey(deviceIndex, featureIndex, funcSwId);

        var tcs = new TaskCompletionSource<CallResult>(TaskCreationOptions.RunContinuationsAsynchronously);
        TimeSpan timeout;
        lock (_gate)
        {
            if (_closed)
                throw new InvalidOperationException("hidpp: connection closed");
            timeout = _timeout;
            _pending[key] = tcs;
        }

        try
        {
            var report = new byte[LongReportLen];
            report[0] = ReportIdLong;
            report[1] = deviceIndex;
            report[2] = featureIndex;
            report[3] = funcSwId;
            parameters.CopyTo(report.AsSpan(4));
            await _stream.WriteAsync(report, ct).ConfigureAwait(false);
            EnsureReaderStarted();

            using var timeoutCts = CancellationTokenSource.CreateLinkedTokenSource(ct);
            timeoutCts.CancelAfter(timeout);
            CallResult result = await tcs.Task.WaitAsync(timeoutCts.Token).ConfigureAwait(false);
            if (result.Error is not null)
                throw result.Error;
            return result.Data;
        }
        catch (OperationCanceledException) when (!ct.IsCancellationRequested)
        {
            throw new TimeoutException(
                $"hidpp: timed out waiting for response (device 0x{deviceIndex:x2}, feature index 0x{featureIndex:x2}, function 0x{function:x})");
        }
        finally
        {
            lock (_gate) _pending.Remove(key);
        }
    }

    private byte AllocSoftwareId()
    {
        lock (_gate)
        {
            byte id = _nextSw;
            _nextSw = (byte)(_nextSw >= 0x0f ? 1 : _nextSw + 1);
            return id;
        }
    }

    /// <summary>
    /// Starts the background reader on first successful write. Dead nodes (writes
    /// fail with EPIPE/EINVAL — e.g. a webcam's interface) never get a reader, so
    /// no thread is ever stranded in an uninterruptible blocking read.
    /// </summary>
    private void EnsureReaderStarted()
    {
        lock (_gate)
        {
            if (_readerStarted || _closed)
                return;
            _readerStarted = true;
            _readTask = Task.Run(ReadLoopAsync);
        }
    }

    /// <summary>
    /// Continuously reads reports, delivering each to whichever call is
    /// waiting, or else to the notifications channel. Reads are gated on
    /// poll(2) with a 500ms timeout: a blocking read() on a char device can't be
    /// cancelled from .NET (Go unblocks these via its netpoller; FileStream's
    /// thread-pool reads cannot), so polling keeps shutdown prompt.
    /// </summary>
    private async Task ReadLoopAsync()
    {
        var buffer = new byte[LongReportLen];
        try
        {
            while (!_cts.IsCancellationRequested)
            {
                var pfd = new PollFd(_fd, PollIn);
                int r;
                while ((r = poll(ref pfd, 1, 500)) < 0 && Marshal.GetLastPInvokeError() == Eintr)
                {
                }
                if (r < 0 || r == 0)
                    continue;
                if ((pfd.Revents & PollIn) == 0)
                    break; // POLLERR / POLLHUP / POLLNVAL — device went away

                int n = _stream.Read(buffer, 0, LongReportLen);
                if (n == 0)
                    break;
                if (n < 4)
                    continue;

                byte reportId = buffer[0];
                if (reportId != ReportIdShort && reportId != ReportIdLong)
                    continue; // not a HID++ report, e.g. a plain HID report on a shared interface
                byte deviceIndex = buffer[1];

                // HID++ 2.0 error responses are always a short report with 0xFF
                // in the feature-index position.
                if (reportId == ReportIdShort && n >= 6 && buffer[2] == 0xff)
                {
                    var errorKey = new CallKey(deviceIndex, buffer[3], buffer[4]);
                    Deliver(errorKey, new CallResult(Array.Empty<byte>(),
                        new HidppErrorException(buffer[3], (byte)(buffer[4] & 0xf0), buffer[5])));
                    continue;
                }

                var key = new CallKey(deviceIndex, buffer[2], buffer[3]);
                if (!Deliver(key, new CallResult(buffer[4..n], null)))
                {
                    _notifications.Writer.TryWrite(new Notification(deviceIndex, buffer[..n]));
                }
            }
        }
        catch (IOException)
        {
        }
        Shutdown();
    }

    private bool Deliver(CallKey key, CallResult result)
    {
        TaskCompletionSource<CallResult>? tcs;
        lock (_gate)
        {
            if (!_pending.TryGetValue(key, out tcs))
                return false;
            _pending.Remove(key);
        }
        tcs.TrySetResult(result);
        return true;
    }

    private void Shutdown()
    {
        List<TaskCompletionSource<CallResult>> toFail;
        lock (_gate)
        {
            if (_closed)
                return;
            _closed = true;
            toFail = [.. _pending.Values];
            _pending.Clear();
        }
        foreach (var tcs in toFail)
            tcs.TrySetException(new IOException("hidpp: connection closed"));
    }

    public async ValueTask DisposeAsync()
    {
        _cts.Cancel();
        if (_readTask is not null)
        {
            try { await _readTask.ConfigureAwait(false); } catch { }
        }
        await _stream.DisposeAsync().ConfigureAwait(false);
        _cts.Dispose();
    }
}
