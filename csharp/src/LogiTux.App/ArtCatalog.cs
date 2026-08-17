using Avalonia.Svg.Skia;
using Avalonia.Threading;

namespace LogiTux.App;

/// <summary>Loads the embedded product-artwork SVGs (originals live in the Go
/// repo's cmd/logitux/assets) as shared IImage values for dashboard cards.
/// SvgImage pins its SvgSource to the creating thread, so loads must happen
/// on the UI thread even when discovery runs on background threads.</summary>
public static class ArtCatalog
{
    private static readonly Dictionary<string, SvgImage?> Cache = [];

    public static SvgImage? Load(string resourceName)
    {
        if (Cache.TryGetValue(resourceName, out var cached))
            return cached;

        return Dispatcher.UIThread.Invoke(() =>
        {
            var source = SvgSource.Load($"avares://LogiTux.App/Assets/{resourceName}", baseUri: null);
            var image = new SvgImage { Source = source };
            Cache[resourceName] = image;
            return image;
        });
    }
}