using System.Diagnostics;
using System.IO;
using System.Windows;
using PianoMidiVisualizationApp;

namespace ReadmeScreenshots;

/// <summary>
/// Usage: ReadmeScreenshots [--check] [--out DIR] [--scale N] [scene ...]
/// Renders every scene (or just the named ones) to DIR/&lt;scene&gt;.png. DIR defaults to
/// site/piano/screenshots in the repository, which the README and the page share, and which
/// are the snapshot test's references too. Exits non-zero if any scene failed or logged a
/// binding error, so a broken screenshot is never committed by accident.
///
/// With --check it writes no reference: it compares each render with DIR/&lt;scene&gt;.png
/// (<see cref="Snapshot"/>) and exits non-zero if any differs, leaving &lt;scene&gt;.new.png and
/// &lt;scene&gt;.diff.png beside it. UPDATE_SNAPSHOTS=1 overrides --check and writes the references,
/// as the plugin editors' snapshot tests do.
/// </summary>
internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        string? outDir = null;
        double scale = 1.5;
        bool check = false;
        var only = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        for (int i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "--check": check = true; break;
                case "--out": outDir = args[++i]; break;
                case "--scale": scale = double.Parse(args[++i], System.Globalization.CultureInfo.InvariantCulture); break;
                default: only.Add(args[i]); break;
            }
        }
        outDir ??= Path.Combine(FindPianoRoot(), "..", "site", "piano", "screenshots");
        outDir = Path.GetFullPath(outDir);
        if (Environment.GetEnvironmentVariable("UPDATE_SNAPSHOTS") is "1" or "true") check = false;

        var unknown = only.Where(name => Scenes.All.All(s => !s.Name.Equals(name, StringComparison.OrdinalIgnoreCase))).ToList();
        if (unknown.Count > 0)
        {
            Console.Error.WriteLine($"Unknown scene(s): {string.Join(", ", unknown)}. Known: {string.Join(", ", Scenes.All.Select(s => s.Name))}");
            return 2;
        }

        // The real App.xaml resources (theme, converters) without running OnStartup, which
        // would open real devices and load the user's settings.
        var app = new App();
        app.InitializeComponent();
        app.ShutdownMode = ShutdownMode.OnExplicitShutdown;

        var bindingErrors = BindingErrorListener.Attach();
        int failures = 0;

        foreach (var scene in Scenes.All.Where(s => only.Count == 0 || only.Contains(s.Name)))
        {
            bindingErrors.Clear();
            var clock = Stopwatch.StartNew();
            try
            {
                using var host = new Host(scene.Width, scene.Height, scene.SetUp);
                scene.Setup(host);

                string path = Path.Combine(outDir, scene.Name + ".png");
                byte[] png = host.Render(scale);
                if (check)
                {
                    var result = Snapshot.Check(png, path);
                    if (!result.Matches) failures++;
                    var line = $"{scene.Name,-16} {clock.Elapsed.TotalSeconds,5:0.0}s  {result.Summary}";
                    if (result.Matches) Console.WriteLine(line);
                    else Console.Error.WriteLine(line);
                }
                else
                {
                    Directory.CreateDirectory(outDir);
                    File.WriteAllBytes(path, png);
                    File.Delete(Path.ChangeExtension(path, ".new.png"));
                    File.Delete(Path.ChangeExtension(path, ".diff.png"));
                    Console.WriteLine($"{scene.Name,-16} {png.Length / 1024,5} KB  {clock.Elapsed.TotalSeconds,5:0.0}s  {path}");
                }
            }
            catch (Exception ex)
            {
                failures++;
                Console.Error.WriteLine($"{scene.Name}: FAILED - {ex}");
            }

            if (bindingErrors.Count > 0)
            {
                failures++;
                Console.Error.WriteLine($"{scene.Name}: {bindingErrors.Count} binding error(s):");
                foreach (var error in bindingErrors.Distinct()) Console.Error.WriteLine("  " + error);
            }
        }

        return failures == 0 ? 0 : 1;
    }

    private static string FindPianoRoot()
    {
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir != null; dir = dir.Parent)
        {
            if (File.Exists(Path.Combine(dir.FullName, "PianoMidiVisualizationApp.sln")))
                return dir.FullName;
        }
        throw new InvalidOperationException("Could not find piano/ (PianoMidiVisualizationApp.sln); pass --out.");
    }
}

/// <summary>Collects WPF binding errors, which otherwise only go to a debugger's output window.</summary>
internal sealed class BindingErrorListener : TraceListener
{
    private readonly List<string> _errors = new();
    private string _partial = "";

    public int Count => _errors.Count;
    public IEnumerable<string> Distinct() => _errors.Distinct();
    public void Clear() => _errors.Clear();

    public static BindingErrorListener Attach()
    {
        var listener = new BindingErrorListener();
        PresentationTraceSources.Refresh();
        PresentationTraceSources.DataBindingSource.Listeners.Add(listener);
        PresentationTraceSources.DataBindingSource.Switch.Level = SourceLevels.Error;
        PresentationTraceSources.ResourceDictionarySource.Listeners.Add(listener);
        PresentationTraceSources.ResourceDictionarySource.Switch.Level = SourceLevels.Error;
        return listener;
    }

    public override void Write(string? message) => _partial += message;

    public override void WriteLine(string? message)
    {
        _errors.Add(_partial + message);
        _partial = "";
    }
}
