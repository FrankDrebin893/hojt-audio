using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using PianoMidiVisualizationApp;
using PianoMidiVisualizationApp.Models;
using PianoMidiVisualizationApp.ViewModels;

namespace ReadmeScreenshots;

/// <summary>
/// One real <see cref="MainWindow"/> over a real <see cref="MainViewModel"/>, with fake MIDI
/// and audio, shown off-screen. Scenes drive it through the view model and the fake keyboard;
/// <see cref="Save"/> renders the window's visual tree to a PNG. Nothing is ever captured from
/// the screen, so whatever else is open on the desktop can never end up in an image.
/// </summary>
internal sealed class Host : IDisposable
{
    public FakeMidiInput Midi { get; } = new();
    public MainViewModel Vm { get; }
    public MainWindow Window { get; }

    public Host(int width, int height)
    {
        Vm = new MainViewModel(Midi, new FakeAudioEngine(), Dispatcher.CurrentDispatcher);
        Vm.RefreshDevices();

        // Fresh defaults, never AppSettings.Load(): the user's own settings file holds their
        // device names, file paths and API key, none of which belongs in a README.
        Vm.ApplySettings(new AppSettings());
        Vm.AutoConnect();

        // Set up as a player would be: a piano sound chosen and audio on, so the title bar's
        // status chip reads "Digital Piano · ASIO" and the empty readout isn't the first-run
        // setup checklist. The file never exists; the fake engine doesn't open it.
        Vm.Settings.SoundFontPath = @"C:\SoundFonts\Salamander Grand Piano\SalamanderGrandPiano.sfz";
        Vm.StartAudioCommand.Execute(null);

        Window = new MainWindow
        {
            DataContext = Vm,
            Width = width,
            Height = height,
            // Shown, because an unshown window builds no visual tree; far off-screen, because
            // it must never flash up on the user's desktop.
            WindowStartupLocation = WindowStartupLocation.Manual,
            Left = -32000,
            Top = -32000,
            ShowInTaskbar = false,
            ShowActivated = false,
        };
        Window.Show();
        Pump(TimeSpan.FromMilliseconds(200));
    }

    // ------------------------------------------------------------------ playing

    public void Press(params int[] notes)
    {
        foreach (var note in notes) Midi.PressKey(note, 88);
        Pump(TimeSpan.FromMilliseconds(40));
    }

    public void Release(params int[] notes)
    {
        foreach (var note in notes) Midi.ReleaseKey(note);
        Pump(TimeSpan.FromMilliseconds(40));
    }

    /// <summary>Holds a chord and saves it to the progression, as Space does.</summary>
    public void SaveChord(params int[] notes)
    {
        Press(notes);
        Vm.SaveCurrentChordCommand.Execute(null);
        Release(notes);
    }

    // ------------------------------------------------------------------ time

    /// <summary>
    /// Runs the dispatcher for a while. Never Thread.Sleep: that starves the dispatcher, and
    /// with it every BeginInvoke and CompositionTarget.Rendering frame the app relies on.
    /// </summary>
    public static void Pump(TimeSpan duration)
    {
        var frame = new DispatcherFrame();
        var timer = new DispatcherTimer(DispatcherPriority.Background) { Interval = duration };
        timer.Tick += (_, _) =>
        {
            timer.Stop();
            frame.Continue = false;
        };
        timer.Start();
        Dispatcher.PushFrame(frame);
    }

    /// <summary>Pumps until a condition holds. Wait on conditions, not guessed delays.</summary>
    public static void PumpUntil(Func<bool> condition, TimeSpan timeout, string what)
    {
        var clock = Stopwatch.StartNew();
        while (!condition())
        {
            if (clock.Elapsed > timeout)
                throw new TimeoutException($"Timed out after {timeout.TotalSeconds:0}s waiting for {what}.");
            Pump(TimeSpan.FromMilliseconds(10));
        }
    }

    // ------------------------------------------------------------------ rendering

    /// <summary>Renders the whole window, title bar included, at <paramref name="scale"/> times 96 DPI.</summary>
    public void Save(string path, double scale)
    {
        Window.UpdateLayout();
        Pump(TimeSpan.FromMilliseconds(100));

        var size = new Size(Window.ActualWidth, Window.ActualHeight);
        var bitmap = new RenderTargetBitmap(
            (int)Math.Ceiling(size.Width * scale), (int)Math.Ceiling(size.Height * scale),
            96 * scale, 96 * scale, PixelFormats.Pbgra32);

        // The window's own background first: with AllowsTransparency, anything not painted by
        // the content would otherwise come out transparent.
        var visual = new DrawingVisual();
        using (var dc = visual.RenderOpen())
        {
            dc.DrawRectangle(Window.Background, null, new Rect(size));
            dc.DrawRectangle(new VisualBrush((Visual)Window.Content)
            {
                Stretch = Stretch.None,
                AlignmentX = AlignmentX.Left,
                AlignmentY = AlignmentY.Top,
                ViewboxUnits = BrushMappingMode.Absolute,
                Viewbox = new Rect(size),
            }, null, new Rect(size));
        }
        bitmap.Render(visual);

        var encoder = new PngBitmapEncoder();
        encoder.Frames.Add(BitmapFrame.Create(bitmap));
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        using var file = File.Create(path);
        encoder.Save(file);
    }

    public void Dispose()
    {
        Window.Close();
        Vm.Dispose();
        Pump(TimeSpan.FromMilliseconds(50));
    }
}
