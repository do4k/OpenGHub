using OpenGHub.Core.Hidpp;

namespace OpenGHub.Core.Devices.Mouse;

public sealed partial class GProMouse
{
    // REPROG_CONTROLS_V4 (0x1b04) function numbers. Ported from
    // internal/device/gpro/buttons.go.
    private const byte ButtonsFuncGetCount = 0x00;
    private const byte ButtonsFuncGetCidInfo = 0x10;
    private const byte ButtonsFuncSetCidReporting = 0x30;

    // Bits from GetCidInfo's capability flags and SetCidReporting's bfield
    // parameter. Both pack a value bit and its adjacent "this bit is being
    // explicitly written" bit; MappingFlagDiverted<<1 is that write-mask bit
    // for the diverted flag specifically.
    private const ushort KeyFlagDivertable = 0x0020;
    private const byte MappingFlagDiverted = 0x01;

    // Maps known control IDs to human-readable labels; unlisted IDs fall back
    // to "Control 0xNNNN". Curated from Solaar's special_keys.py CONTROL
    // table (github.com/pwr-Solaar/Solaar), limited to controls relevant
    // to a mouse.
    private static readonly Dictionary<ushort, string> ControlNames = new()
    {
        [0x0050] = "Left Button",
        [0x0051] = "Right Button",
        [0x0052] = "Middle Button",
        [0x0053] = "Back Button",
        [0x0056] = "Forward Button",
        [0x0059] = "Button 6",
        [0x005a] = "Button 7",
        [0x005b] = "Left Tilt",
        [0x005c] = "Button 8",
        [0x005d] = "Right Tilt",
        [0x00c3] = "Gesture Button",
        [0x00ed] = "DPI Change",
        [0x00fd] = "DPI Switch",
    };

    private static string ControlName(ushort cid) =>
        ControlNames.TryGetValue(cid, out var name) ? name : $"Control 0x{cid:x4}";

    /// <summary>
    /// Resolves REPROG_CONTROLS_V4 (0x1b04); the report-rate equivalent of
    /// RequireFeatureAsync, kept separate because this feature is optional
    /// and its absence must not block the rest of the mouse.
    /// Ported from internal/device/gpro/buttons.go (discoverButtonsFeature).
    /// </summary>
    public static async Task<byte> DiscoverButtonsFeatureIndexAsync(HidppConn conn, byte deviceIndex)
    {
        byte? idx = await HidppRoot.GetFeatureIndexAsync(conn, deviceIndex, FeatureReprogControlsV4).ConfigureAwait(false)
            ?? throw new InvalidOperationException("gpro: device does not support the Reprogrammable Controls feature (0x1b04)");
        return idx.Value;
    }

    /// <summary>
    /// Enumerates the reprogrammable controls via GetControlCount and, per
    /// control index, GetCidInfo; short GetCidInfo responses are skipped the
    /// same way the Go source's discoverButtonsFeature skips them.
    /// Ported from internal/device/gpro/buttons.go (discoverButtonsFeature).
    /// </summary>
    public async Task<IReadOnlyList<ButtonInfo>> GetButtonsAsync()
    {
        if (_buttonsFeatureIndex == 0)
            throw new InvalidOperationException("gpro: device has no reprogrammable controls feature");
        var resp = await _conn.CallAsync(_deviceIndex, _buttonsFeatureIndex, ButtonsFuncGetCount, Array.Empty<byte>()).ConfigureAwait(false);
        if (resp.Length < 1)
            throw new InvalidDataException("gpro: short control count response");

        var controls = new List<ButtonInfo>(resp[0]);
        for (int i = 0; i < resp[0]; i++)
        {
            var info = await _conn.CallAsync(_deviceIndex, _buttonsFeatureIndex, ButtonsFuncGetCidInfo, [(byte)i]).ConfigureAwait(false);
            if (info.Length < 9)
                continue;
            ushort cid = (ushort)((info[0] << 8) | info[1]);
            // Divertable flag is byte 4 | byte 8 (<<8): non-contiguous in the
            // response; replicate the Go bit assembly exactly.
            ushort flags = (ushort)(info[4] | (info[8] << 8));
            controls.Add(new ButtonInfo(cid, ControlName(cid), (flags & KeyFlagDivertable) != 0));
        }
        return controls;
    }

    /// <summary>
    /// SetCidReporting with only the DIVERTED flag touched, and no
    /// device-side remap target (remap=0): Open G Hub always translates
    /// diverted presses itself rather than asking the device to natively
    /// remap to another control.
    /// Ported from internal/device/gpro/buttons.go (setDiversion).
    /// </summary>
    public async Task SetButtonRemapAsync(ushort cid, bool divert)
    {
        if (_buttonsFeatureIndex == 0)
            throw new InvalidOperationException("gpro: device has no reprogrammable controls feature");
        byte bfield = (byte)(MappingFlagDiverted << 1); // the "I am specifying this bit" write-mask bit
        if (divert)
            bfield |= MappingFlagDiverted;
        byte[] payload = [(byte)(cid >> 8), (byte)cid, bfield, 0x00, 0x00];
        await _conn.CallAsync(_deviceIndex, _buttonsFeatureIndex, ButtonsFuncSetCidReporting, payload).ConfigureAwait(false);
    }
}