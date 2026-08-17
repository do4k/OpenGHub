using System.Runtime.InteropServices;

namespace LogiTux.Core.V4l2;

/// <summary>
/// One mmap'd capture buffer from a V4L2 buffer ring: the kernel writes
/// frame data into the region, the driver resumes owning it on QBUF.
/// Ported from internal/v4l2/v4l2.go's mmap path (MapBuffer/UnmapBuffer).
/// </summary>
public sealed class MappedBuffer : IDisposable
{
    [DllImport("libc", SetLastError = true)]
    private static extern IntPtr mmap(IntPtr addr, UIntPtr length, int prot, int flags, int fd, long offset);

    [DllImport("libc", SetLastError = true)]
    private static extern int munmap(IntPtr addr, UIntPtr length);

    private const int ProtReadWrite = 3; // PROT_READ | PROT_WRITE
    private const int MapShared = 1;

    private readonly IntPtr _address;
    private bool _disposed;

    /// <summary>Start of the mapped region.</summary>
    public IntPtr Address => _address;

    /// <summary>Size of the mapped region, in bytes.</summary>
    public int Length { get; }

    private MappedBuffer(IntPtr address, int length)
    {
        _address = address;
        Length = length;
    }

    /// <summary>
    /// Maps buffer index's region (kernel offset and length come from
    /// QUERYBUF) into this process with mmap.
    /// </summary>
    internal static MappedBuffer Map(int fd, long offset, int length, string path)
    {
        IntPtr address = mmap(IntPtr.Zero, (UIntPtr)length, ProtReadWrite, MapShared, fd, offset);
        if (address == (IntPtr)(-1))
            throw new IOException($"v4l2: mmap buffer on {path}: error {Marshal.GetLastPInvokeError()}");
        return new MappedBuffer(address, length);
    }

    /// <summary>
    /// Copies the first count bytes of the region into destination.
    /// </summary>
    public void Read(byte[] destination, int count)
    {
        if (count > Length)
            throw new InvalidDataException($"v4l2: frame of {count} bytes exceeds mapped buffer of {Length}");
        Marshal.Copy(_address, destination, 0, count);
    }

    public void Dispose()
    {
        if (_disposed)
            return;
        _disposed = true;
        munmap(_address, (UIntPtr)Length);
    }
}