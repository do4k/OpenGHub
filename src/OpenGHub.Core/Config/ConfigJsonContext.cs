using System.Text.Json.Serialization;

namespace OpenGHub.Core.Config;

/// <summary>
/// Source-generated JSON metadata for ConfigStore's state table. The
/// trimmed single-file publish disables reflection-based serialization,
/// so the store routes its (de)serialization through this context.
/// </summary>
[JsonSerializable(typeof(Dictionary<string, DeviceState>))]
public sealed partial class ConfigJsonContext : JsonSerializerContext
{
}