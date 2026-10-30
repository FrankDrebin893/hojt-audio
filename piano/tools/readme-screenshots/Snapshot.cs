using System.IO;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace ReadmeScreenshots;

/// <summary>
/// Compares a scene's render with its reference PNG, pixel by pixel, the way the plugin editors'
/// snapshot tests do: on a mismatch it leaves &lt;scene&gt;.new.png (the render) and
/// &lt;scene&gt;.diff.png (what changed, in magenta over a dimmed reference) beside the reference.
/// </summary>
internal static class Snapshot
{
    /// <summary>
    /// How far one colour channel may move (out of 255) before its pixel counts as changed. Small,
    /// for the anti-aliasing of glyph edges, which DirectWrite draws a hair differently from one
    /// Windows build to the next; any change of a colour, size or place moves far more.
    /// </summary>
    public const int ChannelTolerance = 8;

    /// <summary>What one comparison found. <see cref="Matches"/> is the verdict.</summary>
    public sealed record Result(bool Matches, string Summary);

    /// <summary>
    /// Compares <paramref name="png"/> with the PNG at <paramref name="referencePath"/>. Removes
    /// any .new.png and .diff.png an earlier run left there, and writes fresh ones on a mismatch.
    /// </summary>
    public static Result Check(byte[] png, string referencePath)
    {
        string newPath = Path.ChangeExtension(referencePath, ".new.png");
        string diffPath = Path.ChangeExtension(referencePath, ".diff.png");
        File.Delete(newPath);
        File.Delete(diffPath);

        if (!File.Exists(referencePath))
        {
            File.WriteAllBytes(newPath, png);
            return new Result(false, $"no reference at {referencePath}; wrote {Path.GetFileName(newPath)}");
        }

        var actual = Pixels.Decode(png);
        var expected = Pixels.Decode(File.ReadAllBytes(referencePath));

        int width = Math.Max(actual.Width, expected.Width);
        int height = Math.Max(actual.Height, expected.Height);
        var diff = new byte[width * height * 4];
        int changed = 0, worst = 0;

        for (int y = 0; y < height; y++)
        {
            for (int x = 0; x < width; x++)
            {
                int o = (y * width + x) * 4;
                bool inActual = x < actual.Width && y < actual.Height;
                bool inExpected = x < expected.Width && y < expected.Height;

                int delta = 255;
                if (inActual && inExpected)
                {
                    int a = actual.Offset(x, y), e = expected.Offset(x, y);
                    delta = 0;
                    for (int c = 0; c < 4; c++)
                        delta = Math.Max(delta, Math.Abs(actual.Bgra[a + c] - expected.Bgra[e + c]));
                }
                worst = Math.Max(worst, delta);

                if (delta > ChannelTolerance)
                {
                    changed++;
                    diff[o + 0] = 0xD0; diff[o + 1] = 0x20; diff[o + 2] = 0xFF; diff[o + 3] = 0xFF;   // magenta
                }
                else
                {
                    // The reference, greyed and dimmed to a third, so the magenta reads as where it is.
                    int e = expected.Offset(x, y);
                    byte grey = (byte)((expected.Bgra[e] * 0.114 + expected.Bgra[e + 1] * 0.587 + expected.Bgra[e + 2] * 0.299)
                                       * expected.Bgra[e + 3] / 255 / 3);
                    diff[o + 0] = grey; diff[o + 1] = grey; diff[o + 2] = grey; diff[o + 3] = 0xFF;
                }
            }
        }

        bool sameSize = actual.Width == expected.Width && actual.Height == expected.Height;
        if (sameSize && changed == 0)
            return new Result(true, worst == 0 ? "matches" : $"matches (within {worst}/255)");

        File.WriteAllBytes(newPath, png);
        File.WriteAllBytes(diffPath, Pixels.Encode(diff, width, height));

        string size = sameSize ? "" : $"size {actual.Width}x{actual.Height}, reference {expected.Width}x{expected.Height}; ";
        return new Result(false,
            $"{size}{changed} of {width * height} pixels changed by more than {ChannelTolerance}/255 " +
            $"(at most {worst}); wrote {Path.GetFileName(newPath)} and {Path.GetFileName(diffPath)}");
    }

    /// <summary>A PNG decoded to straight (not premultiplied) BGRA bytes, top row first.</summary>
    private sealed record Pixels(int Width, int Height, byte[] Bgra)
    {
        public int Offset(int x, int y) => (y * Width + x) * 4;

        public static Pixels Decode(byte[] png)
        {
            using var stream = new MemoryStream(png);
            var decoder = new PngBitmapDecoder(stream, BitmapCreateOptions.PreservePixelFormat, BitmapCacheOption.OnLoad);
            var frame = new FormatConvertedBitmap(decoder.Frames[0], PixelFormats.Bgra32, null, 0);
            var bytes = new byte[frame.PixelWidth * frame.PixelHeight * 4];
            frame.CopyPixels(bytes, frame.PixelWidth * 4, 0);
            return new Pixels(frame.PixelWidth, frame.PixelHeight, bytes);
        }

        public static byte[] Encode(byte[] bgra, int width, int height)
        {
            var bitmap = BitmapSource.Create(width, height, 96, 96, PixelFormats.Bgra32, null, bgra, width * 4);
            var encoder = new PngBitmapEncoder();
            encoder.Frames.Add(BitmapFrame.Create(bitmap));
            using var stream = new MemoryStream();
            encoder.Save(stream);
            return stream.ToArray();
        }
    }
}
