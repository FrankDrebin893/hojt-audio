using System.Diagnostics;
using System.IO;
using System.Windows;
using PianoMidiVisualizationApp;

namespace ReadmeScreenshots;

/// <summary>
/// Usage: ReadmeScreenshots [--out DIR] [--scale N] [scene ...]
/// Renders every scene (or just the named ones) to DIR/&lt;scene&gt;.png. DIR defaults to
/// docs/screenshots in the repository. Exits non-zero if any scene failed or logged a
/// binding error, so a broken screenshot is never committed by accident.
/// </summary>
internal static class Program
{
    [STAThread]
    private static int Main(string[] args)
    {
        string? outDir = null;
        double scale = 1.5;
        var only = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        for (int i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "--out": outDir = args[++i]; break;
                case "--scale": scale = double.Parse(args[++i], System.Globalization.CultureInfo.InvariantCulture); break;
                default: only.Add(args[i]); break;
            }
        }
        outDir ??= Path.Combine(FindRepoRoot(), "docs", "screenshots");

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
                host.Save(path, scale);
                Console.WriteLine($"{scene.Name,-16} {new FileInfo(path).Length / 1024,5} KB  {clock.Elapsed.TotalSeconds,5:0.0}s  {path}");
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

    private static string FindRepoRoot()
    {
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir != null; dir = dir.Parent)
        {
            if (File.Exists(Path.Combine(dir.FullName, "PianoMidiVisualizationApp.sln")))
                return dir.FullName;
        }
        throw new InvalidOperationException("Could not find the repository root; pass --out.");
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
