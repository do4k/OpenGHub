using LogiTux.Core.Devices.Camera;
using LogiTux.Core.Devices.Headset;
using LogiTux.Core.Devices.Litra;
using LogiTux.Core.Devices.Mouse;
using LogiTux.Core.Hid;

namespace LogiTux.App.ViewModels;

/// <summary>Maps enumerated Logitech HID devices to dashboard cards, mirroring
/// internal/device's plugin registry.</summary>
public static class DeviceManager
{
    public const ushort VendorId = 0x046d;

    private static readonly Dictionary<ushort, string> ProductNames = new()
    {
        [LitraLight.ProductGlow] = "Litra Glow",
        [LitraLight.ProductBeam] = "Litra Beam",
        [ProxHeadset.ProductId] = "PRO X Wireless Headset",
        [GProMouse.ProductReceiver] = "Lightspeed receiver",
        [GProMouse.ProductWired] = "G Pro Wireless",
        [GProMouse.ProductWiredUsb] = "G Pro Wireless (wired)",
        [GProMouse.ProductSuperlight] = "PRO X Superlight",
        [GProMouse.ProductSuperlight2] = "PRO X Superlight 2",
    };

    private static readonly Dictionary<ushort, string> ProductArt = new()
    {
        [LitraLight.ProductGlow] = "litra-glow.svg",
        [LitraLight.ProductBeam] = "litra-beam.svg",
        [ProxHeadset.ProductId] = "prox.svg",
        [GProMouse.ProductWired] = "gpro.svg",
        [GProMouse.ProductWiredUsb] = "gpro.svg",
        [GProMouse.ProductSuperlight] = "gpro.svg",
        [GProMouse.ProductSuperlight2] = "gpro.svg",
    };

    public static async Task<IReadOnlyList<DeviceCardViewModel>> DiscoverAsync(HidBackend backend)
    {
        var cards = new List<DeviceCardViewModel>();
        foreach (DeviceInfo info in backend.Enumerate(VendorId))
        {
            string name = ProductNames.GetValueOrDefault(info.ProductId) ?? $"Logitech ({info.ProductId:x4})";
            string subtitle = $"serial {info.Serial ?? "n/a"} · {info.Path}";
            try
            {
                if (info.ProductId is LitraLight.ProductGlow or LitraLight.ProductBeam)
                {
                    cards.Add(LitraViewModel.Create(backend, info));
                }
                else if (info.ProductId == ProxHeadset.ProductId)
                {
                    HeadsetViewModel? headset = await HeadsetViewModel.CreateAsync(backend, info).ConfigureAwait(false);
                    if (headset is not null)
                        cards.Add(headset);
                }
                else if (GProMouse.IsMouseProduct(info.ProductId))
                {
                    try
                    {
                        MouseViewModel? mouse = await MouseViewModel.CreateAsync(backend, info).ConfigureAwait(false);
                        if (mouse is not null)
                            cards.Add(mouse);
                    }
                    catch (InvalidOperationException)
                    {
                        // "ping direct-connected mouse: no response" etc. — the
                        // mouse isn't plugged in right now; its tile would be
                        // pure noise. Skip it silently.
                    }
                }
            }
            catch (Exception e)
            {
                cards.Add(new ErrorCardViewModel(
                    name, subtitle, e.Message,
                    ProductArt.GetValueOrDefault(info.ProductId) is string art ? ArtCatalog.Load(art) : null));
            }
        }

        foreach (string node in V4l2Camera.EnumerateNodes())
        {
            try
            {
                CameraCardViewModel? card = CameraCardViewModel.OpenAsync(node);
                if (card is not null)
                    cards.Add(card);
            }
            catch (Exception e)
            {
                cards.Add(new ErrorCardViewModel(
                    V4l2Camera.ProductNameForNode(node) ?? "Webcam", $"webcam node /dev/{node}", e.Message, null));
            }
        }
        return cards;
    }
}