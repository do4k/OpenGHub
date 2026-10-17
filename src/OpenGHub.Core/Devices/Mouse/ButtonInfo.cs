namespace OpenGHub.Core.Devices.Mouse;

/// <summary>
/// One reprogrammable control discovered via GetCidInfo.
/// Ported from internal/device/gpro/buttons.go (buttonControl).
/// </summary>
public sealed record ButtonInfo(ushort Id, string Name, bool IsDivertable);