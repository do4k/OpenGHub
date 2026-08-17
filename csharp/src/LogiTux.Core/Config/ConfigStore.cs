using System.Text.Json;

namespace LogiTux.Core.Config;

/// <summary>
/// JSON-backed, serial-number-keyed table of DeviceState, persisted to
/// disk on every Set. Writes are atomic (temp file in the same directory,
/// then rename over the target) so a crash mid-write can't leave a
/// truncated state file.
/// Ported from internal/config/config.go.
/// </summary>
public sealed class ConfigStore
{
    /// <summary>
    /// Gets the path of the state file this store is backed by.
    /// </summary>
    public string Path { get; }

    private readonly object _lock = new();
    private readonly Dictionary<string, DeviceState> _states = [];

    private static readonly JsonSerializerOptions JsonOpts = new()
    {
        WriteIndented = true,
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
    };

    private ConfigStore(string path)
    {
        Path = path;
    }

    /// <summary>
    /// Returns the standard location for LogiTux's state file, honoring
    /// XDG_CONFIG_HOME with the usual ~/.config fallback.
    /// </summary>
    public static string DefaultPath()
    {
        var configHome = Environment.GetEnvironmentVariable("XDG_CONFIG_HOME")
            ?? System.IO.Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".config");
        return System.IO.Path.Combine(configHome, "logitux", "state.json");
    }

    /// <summary>
    /// Loads the state file at path, if it exists, and returns a store
    /// backed by it. A missing or empty file is not an error; the store
    /// starts empty and creates the file on the first Set.
    /// </summary>
    /// <exception cref="IOException">Thrown when the file exists but
    /// cannot be read.</exception>
    /// <exception cref="InvalidDataException">Thrown when the file exists
    /// but its JSON cannot be parsed.</exception>
    public static ConfigStore Open(string path)
    {
        var store = new ConfigStore(path);

        if (!File.Exists(path))
        {
            return store;
        }

        byte[] data;
        try
        {
            data = File.ReadAllBytes(path);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            throw new IOException($"config: read {path}: {e.Message}", e);
        }
        if (data.Length == 0)
        {
            return store;
        }

        try
        {
            var states = JsonSerializer.Deserialize<Dictionary<string, DeviceState>>(data, JsonOpts);
            if (states is not null)
            {
                store._states.Clear();
                foreach (var kvp in states)
                {
                    store._states[kvp.Key] = kvp.Value;
                }
            }
        }
        catch (JsonException e)
        {
            throw new InvalidDataException($"config: parse {path}: {e.Message}", e);
        }

        return store;
    }

    /// <summary>
    /// Returns the last-known state for serial, if any.
    /// </summary>
    public bool TryGet(string serial, out DeviceState state)
    {
        lock (_lock)
        {
            return _states.TryGetValue(serial, out state!);
        }
    }

    /// <summary>
    /// Records state for serial and persists the whole store to disk
    /// atomically.
    /// </summary>
    /// <exception cref="IOException">Thrown when any step of the write
    /// fails; the exception message names the failing step.</exception>
    public void Set(string serial, DeviceState state)
    {
        lock (_lock)
        {
            _states[serial] = state;
            SaveLocked();
        }
    }

    // saveLocked writes the store to disk atomically (write to a temp file
    // in the same directory, then rename over the target). Callers must
    // hold _lock.
    private void SaveLocked()
    {
        var dir = System.IO.Path.GetDirectoryName(Path)
            ?? throw new IOException($"config: resolve directory of {Path}");
        try
        {
            Directory.CreateDirectory(dir);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            throw new IOException($"config: create {dir}: {e.Message}", e);
        }

        var data = JsonSerializer.SerializeToUtf8Bytes(_states, JsonOpts);
        var tmpPath = System.IO.Path.Combine(dir, $".state-{System.IO.Path.GetRandomFileName()}.json.tmp");

        try
        {
            File.WriteAllBytes(tmpPath, data);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            TryRemove(tmpPath);
            throw new IOException($"config: write temp file: {e.Message}", e);
        }

        try
        {
            File.Move(tmpPath, Path, overwrite: true);
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            TryRemove(tmpPath);
            throw new IOException($"config: rename into place: {e.Message}", e);
        }
    }

    private static void TryRemove(string path)
    {
        try
        {
            File.Delete(path);
        }
        catch (IOException)
        {
            // Best-effort cleanup; a stray temp file is harmless.
        }
    }
}