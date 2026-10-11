using System;
using System.Collections.Generic;
using System.Collections.Specialized;
using System.Drawing;
using System.Drawing.Imaging;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Text;

namespace RefGen
{
    // Runs the ORIGINAL Rasterbator engine (from the shipped Rasterbator.exe,
    // referenced as an assembly) with the iTextSharp 1.0.4.0 build it was
    // compiled against. Generates reference PDFs for comparing with the Rust port.
    public static class Program
    {
        public static int Main(string[] args)
        {
            if (args.Length == 0) return Usage();
            try
            {
                switch (args[0])
                {
                    case "make-images": return MakeImages(ArgMap(args.Skip(1)));
                    case "render": return Render(ArgMap(args.Skip(1)));
                    default: return Usage();
                }
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine("ERROR: " + ex);
                return 99;
            }
        }

        private static int Usage()
        {
            Console.WriteLine("refgen make-images --out <dir>");
            Console.WriteLine("refgen render --input <img> --output <pdf> --iw <mm> --ih <mm> --pw <mm> --ph <mm> --dot <mm> --color <black|avg|RRGGBB> --cutout <bool> [--ow <px> --oh <px> --crop x1,y1,x2,y2]");
            return 2;
        }

        private static Dictionary<string, string> ArgMap(IEnumerable<string> args)
        {
            var map = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            string key = null;
            foreach (var a in args)
            {
                if (key == null)
                {
                    if (!a.StartsWith("--")) throw new ArgumentException("expected --key, got " + a);
                    key = a.Substring(2);
                }
                else
                {
                    map[key] = a;
                    key = null;
                }
            }
            if (key != null) throw new ArgumentException("missing value for --" + key);
            return map;
        }

        private static string Get(Dictionary<string, string> m, string k, string def = null)
        {
            string v;
            if (m.TryGetValue(k, out v)) return v;
            if (def != null) return def;
            throw new ArgumentException("missing --" + k);
        }

        private static int MakeImages(Dictionary<string, string> m)
        {
            string dir = Get(m, "out");
            Directory.CreateDirectory(dir);

            using (var bmp = new Bitmap(600, 400, PixelFormat.Format24bppRgb))
            {
                for (int y = 0; y < 400; y++)
                    for (int x = 0; x < 600; x++)
                    {
                        int g = x * 255 / 599;
                        int v = y * 60 / 399;
                        bmp.SetPixel(x, y, Color.FromArgb(Math.Min(255, g + v / 3), Math.Min(255, g + v / 2), Math.Min(255, g + v)));
                    }
                bmp.Save(Path.Combine(dir, "gradient.png"), ImageFormat.Png);
            }

            using (var bmp = new Bitmap(800, 600, PixelFormat.Format24bppRgb))
            {
                var rnd = new Random(12345);
                for (int y = 0; y < 600; y++)
                    for (int x = 0; x < 800; x++)
                    {
                        double r = 128 + 120 * Math.Sin(x * 0.017) * Math.Cos(y * 0.011);
                        int n = rnd.Next(-18, 19);
                        bmp.SetPixel(x, y, Color.FromArgb(Clamp(r + n), Clamp(r * 0.7 + n), Clamp(r * 0.4 + n)));
                    }
                SaveJpeg(bmp, Path.Combine(dir, "photo.jpg"), 90L);
            }

            using (var bmp = new Bitmap(400, 300, PixelFormat.Format24bppRgb))
            {
                using (var g = Graphics.FromImage(bmp)) g.Clear(Color.FromArgb(0x33, 0x66, 0xCC));
                bmp.Save(Path.Combine(dir, "solid.png"), ImageFormat.Png);
            }

            using (var bmp = new Bitmap(256, 256, PixelFormat.Format32bppArgb))
            {
                for (int y = 0; y < 256; y++)
                    for (int x = 0; x < 256; x++)
                        bmp.SetPixel(x, y, Color.FromArgb(255 - y, x, 255 - x, 128));
                bmp.Save(Path.Combine(dir, "alpha.png"), ImageFormat.Png);
            }

            using (var bmp = new Bitmap(300, 900, PixelFormat.Format24bppRgb))
            {
                for (int y = 0; y < 900; y++)
                    for (int x = 0; x < 300; x++)
                    {
                        int v = y * 255 / 899;
                        bmp.SetPixel(x, y, Color.FromArgb(v, v, v));
                    }
                bmp.Save(Path.Combine(dir, "tall.png"), ImageFormat.Png);
            }

            using (var bmp = new Bitmap(320, 240, PixelFormat.Format8bppIndexed))
            {
                var pal = bmp.Palette;
                for (int i = 0; i < 256; i++) pal.Entries[i] = Color.FromArgb(255, i, 255 - i, i * 7 % 256);
                bmp.Palette = pal;
                var data = bmp.LockBits(new Rectangle(0, 0, 320, 240), ImageLockMode.WriteOnly, PixelFormat.Format8bppIndexed);
                for (int y = 0; y < 240; y++)
                {
                    var row = new byte[data.Stride];
                    for (int x = 0; x < 320; x++) row[x] = (byte)((x + y) % 256);
                    System.Runtime.InteropServices.Marshal.Copy(row, 0, data.Scan0 + y * data.Stride, data.Stride);
                }
                bmp.UnlockBits(data);
                bmp.Save(Path.Combine(dir, "palette.gif"), ImageFormat.Gif);
            }

            using (var bmp = new Bitmap(40, 40, PixelFormat.Format24bppRgb))
            {
                using (var g = Graphics.FromImage(bmp)) g.Clear(Color.Black);
                bmp.Save(Path.Combine(dir, "tiny.png"), ImageFormat.Png);
            }

            Console.WriteLine("images written to " + Path.GetFullPath(dir));
            return 0;
        }

        private static int Clamp(double v) { int i = (int)Math.Round(v); return i < 0 ? 0 : (i > 255 ? 255 : i); }

        private static void SaveJpeg(Bitmap bmp, string path, long quality)
        {
            var codec = ImageCodecInfo.GetImageEncoders().First(c => c.FormatID == ImageFormat.Jpeg.Guid);
            var ep = new EncoderParameters(1);
            ep.Param[0] = new EncoderParameter(System.Drawing.Imaging.Encoder.Quality, quality);
            bmp.Save(path, codec, ep);
        }

        private static int Render(Dictionary<string, string> m)
        {
            string input = Get(m, "input");
            string output = Get(m, "output");
            int iw = (int)double.Parse(Get(m, "iw"), CultureInfo.InvariantCulture);
            int ih = (int)double.Parse(Get(m, "ih"), CultureInfo.InvariantCulture);
            int pw = (int)double.Parse(Get(m, "pw"), CultureInfo.InvariantCulture);
            int ph = (int)double.Parse(Get(m, "ph"), CultureInfo.InvariantCulture);
            string dot = Get(m, "dot");
            string color = Get(m, "color", "black");
            bool cutout = bool.Parse(Get(m, "cutout", "false"));

            using (var bmp = new Bitmap(input))
            {
                int ow = bmp.Width, oh = bmp.Height;
                if (m.ContainsKey("ow")) ow = int.Parse(Get(m, "ow"), CultureInfo.InvariantCulture);
                if (m.ContainsKey("oh")) oh = int.Parse(Get(m, "oh"), CultureInfo.InvariantCulture);
                int x1 = 0, y1 = 0, x2 = ow, y2 = oh;
                if (m.ContainsKey("crop"))
                {
                    var p = Get(m, "crop").Split(',');
                    x1 = int.Parse(p[0]); y1 = int.Parse(p[1]); x2 = int.Parse(p[2]); y2 = int.Parse(p[3]);
                }

                var attr = new NameValueCollection();
                attr["ImageWidth"] = iw.ToString(CultureInfo.InvariantCulture);
                attr["ImageHeight"] = ih.ToString(CultureInfo.InvariantCulture);
                attr["PaperWidth"] = pw.ToString(CultureInfo.InvariantCulture);
                attr["PaperHeight"] = ph.ToString(CultureInfo.InvariantCulture);
                attr["RasterSize"] = dot;
                attr["CroppingRectangle"] = cutout ? "True" : "False";
                attr["RasterColor"] = color == "black" ? "000000" : color;
                attr["OriginalImageWidth"] = ow.ToString(CultureInfo.InvariantCulture);
                attr["OriginalImageHeight"] = oh.ToString(CultureInfo.InvariantCulture);
                attr["DisplayImageWidth"] = ow.ToString(CultureInfo.InvariantCulture);
                attr["DisplayImageHeight"] = oh.ToString(CultureInfo.InvariantCulture);
                attr["x1"] = x1.ToString(CultureInfo.InvariantCulture);
                attr["y1"] = y1.ToString(CultureInfo.InvariantCulture);
                attr["x2"] = x2.ToString(CultureInfo.InvariantCulture);
                attr["y2"] = y2.ToString(CultureInfo.InvariantCulture);
                attr["OriginalFilename"] = Path.GetFullPath(input);
                attr["TargetFilename"] = Path.GetFullPath(output);
                attr["WorkDirectory"] = Path.GetDirectoryName(Path.GetFullPath(output));

                var rb = new global::Rasterbator.Rasterbator(attr);
                rb.Go();
                if (!string.IsNullOrEmpty(rb.ErrorMessage))
                {
                    Console.Error.WriteLine("engine error: " + rb.ErrorMessage);
                    return 3;
                }
            }
            Console.WriteLine("ok " + output);
            return 0;
        }
    }
}
