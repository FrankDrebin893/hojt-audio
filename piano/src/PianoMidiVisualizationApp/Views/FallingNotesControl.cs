using System.ComponentModel;
using System.Globalization;
using System.Windows;
using System.Windows.Media;
using PianoMidiVisualizationApp.Services.SongPractice;
using PianoMidiVisualizationApp.ViewModels;

namespace PianoMidiVisualizationApp.Views;

/// <summary>A note as last drawn: which key column it fell in, in this control's coordinates.</summary>
/// <param name="Note">The note as written in the song.</param>
/// <param name="Key">The key it lands on: the same note, or folded by octaves into the keyboard's range.</param>
/// <param name="Column">The key's full column, before the note's inset — directly comparable to the key.</param>
public readonly record struct DrawnNote(int Note, int Key, Rect Column, Rect Bounds);

/// <summary>
/// Song notes falling toward the keyboard below: each note in its key's column, its bottom
/// edge reaching the bottom of this control exactly as the song clock reaches its start.
/// </summary>
/// <remarks>
/// <para>Horizontal placement is not recomputed from constants: it maps
/// <see cref="KeyboardLayout"/> x through the keyboard control's actual transform into this
/// control, so the columns line up with the keys at every window size, whatever the keyboard's
/// Viewbox, margins or alignment do.</para>
/// <para>Drawn in OnRender, since it redraws every frame while playing.</para>
/// </remarks>
public class FallingNotesControl : FrameworkElement
{
    /// <summary>How much song time the height of the view shows.</summary>
    public static readonly TimeSpan VisibleSpan = TimeSpan.FromSeconds(3.5);

    /// <summary>Distinct part colours in the theme (Brush.Song.Part0..N-1); indexes wrap.</summary>
    public const int PartColorCount = 6;

    private const double MinimumNoteHeight = 3;

    private static readonly Typeface LabelTypeface = new("Segoe UI");

    public static readonly DependencyProperty SessionProperty = DependencyProperty.Register(
        nameof(Session), typeof(SongPracticeViewModel), typeof(FallingNotesControl),
        new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.AffectsRender, OnSessionChanged));

    public static readonly DependencyProperty KeyboardProperty = DependencyProperty.Register(
        nameof(Keyboard), typeof(PianoKeyboardControl), typeof(FallingNotesControl),
        new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.AffectsRender, OnKeyboardChanged));

    public SongPracticeViewModel? Session
    {
        get => (SongPracticeViewModel?)GetValue(SessionProperty);
        set => SetValue(SessionProperty, value);
    }

    /// <summary>The keyboard the notes fall onto. Its layout and on-screen transform place every column.</summary>
    public PianoKeyboardControl? Keyboard
    {
        get => (PianoKeyboardControl?)GetValue(KeyboardProperty);
        set => SetValue(KeyboardProperty, value);
    }

    private readonly List<DrawnNote> _drawnNotes = new();

    /// <summary>What the last render drew, for tests that check notes against their keys.</summary>
    public IReadOnlyList<DrawnNote> DrawnNotes => _drawnNotes;

    /// <summary>The last mapping drawn with, so a layout pass that moves the keyboard triggers a redraw.</summary>
    private (double Offset, double Scale) _lastMapping;

    private Palette? _palette;

    public FallingNotesControl()
    {
        ClipToBounds = true;
        SnapsToDevicePixels = true;
    }

    private static void OnSessionChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        var control = (FallingNotesControl)d;
        if (e.OldValue is SongPracticeViewModel oldSession)
        {
            oldSession.FrameAdvanced -= control.OnFrameAdvanced;
            oldSession.PropertyChanged -= control.OnSessionPropertyChanged;
        }
        if (e.NewValue is SongPracticeViewModel newSession)
        {
            newSession.FrameAdvanced += control.OnFrameAdvanced;
            newSession.PropertyChanged += control.OnSessionPropertyChanged;
        }
    }

    private static void OnKeyboardChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        var control = (FallingNotesControl)d;
        if (e.OldValue is PianoKeyboardControl oldKeyboard)
            oldKeyboard.LayoutUpdated -= control.OnKeyboardLayoutUpdated;
        if (e.NewValue is PianoKeyboardControl newKeyboard)
            newKeyboard.LayoutUpdated += control.OnKeyboardLayoutUpdated;
    }

    private void OnFrameAdvanced(object? sender, EventArgs e) => InvalidateVisual();

    private void OnSessionPropertyChanged(object? sender, PropertyChangedEventArgs e) => InvalidateVisual();

    /// <summary>
    /// The keyboard can rescale without this control changing size (its row is sized to it),
    /// so any layout pass that moves the keys has to redraw the columns.
    /// </summary>
    private void OnKeyboardLayoutUpdated(object? sender, EventArgs e)
    {
        if (TryGetKeyMapping(out double offset, out double scale)
            && (Math.Abs(offset - _lastMapping.Offset) > 0.01 || Math.Abs(scale - _lastMapping.Scale) > 1e-6))
        {
            InvalidateVisual();
        }
    }

    /// <summary>
    /// Maps the keyboard's intrinsic x (as <see cref="KeyboardLayout"/> gives it) into this
    /// control: <c>x = offset + intrinsicX × scale</c>. False until both are laid out.
    /// </summary>
    public bool TryGetKeyMapping(out double offset, out double scale)
    {
        offset = 0;
        scale = 0;

        if (Keyboard is not { } keyboard || !keyboard.IsVisible || !IsVisible) return false;
        if (PresentationSource.FromVisual(keyboard) == null || PresentationSource.FromVisual(this) == null) return false;

        GeneralTransform transform;
        try
        {
            transform = keyboard.TransformToVisual(this);
        }
        catch (InvalidOperationException)
        {
            return false;   // not sharing a visual tree yet
        }

        double width = keyboard.Layout.TotalWidth;
        var left = transform.Transform(new Point(0, 0));
        var right = transform.Transform(new Point(width, 0));
        scale = (right.X - left.X) / width;
        offset = left.X;
        return scale > 0;
    }

    /// <summary>A key's full column in this control's coordinates, from a mapping.</summary>
    private static Rect KeyColumn(KeyboardLayout layout, int key, double offset, double scale, double height) =>
        new(offset + (layout.KeyLeft(key) * scale), 0, KeyboardLayout.KeyWidth(key) * scale, height);

    protected override void OnRender(DrawingContext dc)
    {
        _drawnNotes.Clear();

        double width = ActualWidth, height = ActualHeight;
        if (width <= 0 || height <= 0) return;

        _palette ??= new Palette(this);
        var p = _palette;

        if (Keyboard is not { } keyboard || !TryGetKeyMapping(out double offset, out double scale)) return;
        _lastMapping = (offset, scale);

        var layout = keyboard.Layout;
        double laneLeft = offset;
        double laneRight = offset + (layout.TotalWidth * scale);

        dc.DrawRectangle(p.Lane, null, new Rect(laneLeft, 0, laneRight - laneLeft, height));
        DrawKeyGroupLines(dc, layout, offset, scale, height);

        if (Session is not { Song: { } song } session) return;

        var position = session.Clock.Position;
        double pixelsPerSecond = height / VisibleSpan.TotalSeconds;
        double Y(TimeSpan t) => height - ((t - position).TotalSeconds * pixelsPerSecond);

        var windowEnd = position + VisibleSpan;
        bool looping = session.IsLoopEnabled;
        var loopStart = session.LoopStartTime;
        var loopEnd = session.LoopEndTime;

        DrawBarLines(dc, song, position, windowEnd, looping, loopStart, loopEnd, Y, laneLeft, laneRight);

        // While looping, nothing after the loop end will play: the loop start is drawn coming
        // round again in its place, so what falls is always what will be heard next.
        var drawUntil = looping && loopEnd < windowEnd ? loopEnd : windowEnd;
        DrawNotes(dc, session, layout, offset, scale, height, position, drawUntil, TimeSpan.Zero,
                  startingFrom: null, dimBefore: looping ? loopStart : null, Y);

        if (looping && loopEnd < windowEnd && loopEnd > loopStart)
        {
            var wrap = loopEnd - loopStart;
            var wrapPosition = position - wrap;
            DrawNotes(dc, session, layout, offset, scale, height, wrapPosition, windowEnd - wrap, wrap,
                      startingFrom: loopStart, dimBefore: null, Y);
        }

        if (looping)
            DrawLoopMarkers(dc, loopStart, loopEnd, position, windowEnd, Y, laneLeft, laneRight, height);

        // The line the notes land on: the top edge of the keys, in spirit.
        dc.DrawRectangle(p.HitLine, null, new Rect(laneLeft, height - 2, laneRight - laneLeft, 2));
    }

    /// <summary>A faint line at every C and F: the two groups of black keys, so columns are easy to follow.</summary>
    private void DrawKeyGroupLines(DrawingContext dc, KeyboardLayout layout, double offset, double scale, double height)
    {
        var p = _palette!;
        for (int note = layout.LowestNote + 1; note <= layout.HighestNote; note++)
        {
            int pitchClass = note % 12;
            if (pitchClass is not (0 or 5)) continue;

            // The seam just left of the key, i.e. the white-key slot boundary.
            double x = Math.Round(offset + (layout.KeyLeft(note) * scale)) - 0.5;
            dc.DrawLine(pitchClass == 0 ? p.OctaveLine : p.GroupLine, new Point(x, 0), new Point(x, height));
        }
    }

    private void DrawBarLines(DrawingContext dc, Song song, TimeSpan position, TimeSpan windowEnd,
                              bool looping, TimeSpan loopStart, TimeSpan loopEnd,
                              Func<TimeSpan, double> y, double laneLeft, double laneRight)
    {
        var p = _palette!;
        double pixelsPerDip = VisualTreeHelper.GetDpi(this).PixelsPerDip;

        void Draw(int bar, TimeSpan drawnAt)
        {
            double lineY = Math.Round(y(drawnAt)) + 0.5;
            dc.DrawLine(p.BarLine, new Point(laneLeft, lineY), new Point(laneRight, lineY));

            var label = new FormattedText(bar.ToString(CultureInfo.InvariantCulture), CultureInfo.InvariantCulture,
                                          FlowDirection.LeftToRight, LabelTypeface, 10, p.BarNumber, pixelsPerDip);
            dc.DrawText(label, new Point(laneLeft + 4, lineY - label.Height - 1));
        }

        var end = looping && loopEnd < windowEnd ? loopEnd : windowEnd;
        for (int bar = song.BarAt(position); bar <= song.BarCount; bar++)
        {
            var start = song.BarStart(bar);
            if (start > end) break;
            if (start >= position) Draw(bar, start);
        }

        if (looping && loopEnd < windowEnd)
        {
            var wrap = loopEnd - loopStart;
            for (int bar = song.BarAt(loopStart); bar <= song.BarCount; bar++)
            {
                var start = song.BarStart(bar);
                if (start < loopStart) continue;
                if (start + wrap > windowEnd) break;
                Draw(bar, start + wrap);
            }
        }
    }

    /// <summary>
    /// Draws the visible notes starting before <paramref name="until"/> and still sounding at
    /// <paramref name="from"/>, shifted later by <paramref name="shift"/> (for the loop's next pass).
    /// Notes starting before <paramref name="startingFrom"/> are skipped: a loop only replays
    /// notes that start inside it. Notes before <paramref name="dimBefore"/> are the lead-in's
    /// run-up and are drawn faded.
    /// </summary>
    private void DrawNotes(DrawingContext dc, SongPracticeViewModel session, KeyboardLayout layout,
                           double offset, double scale, double height,
                           TimeSpan from, TimeSpan until, TimeSpan shift,
                           TimeSpan? startingFrom, TimeSpan? dimBefore, Func<TimeSpan, double> y)
    {
        var p = _palette!;
        var notes = session.VisibleNotes;
        var roles = session.PartRoles;
        var parts = session.Song!.Parts;
        var now = session.Clock.Position;

        int first = FirstStartingAtOrAfter(notes, from - session.LongestVisibleNote);
        for (int i = first; i < notes.Count; i++)
        {
            var note = notes[i];
            if (note.Start >= until) break;
            if (note.End <= from || note.Start < startingFrom) continue;

            int key = layout.Fold(note.Note);
            var column = KeyColumn(layout, key, offset, scale, height);

            double bottom = y(note.Start + shift);
            double top = y(note.End + shift);
            if (bottom - top < MinimumNoteHeight) top = bottom - MinimumNoteHeight;
            if (bottom < 0 || top > height) continue;

            bool black = KeyboardLayout.IsBlack(key);
            double inset = (black ? 1 : 2) * scale;
            var bounds = new Rect(column.X + inset, top, Math.Max(1, column.Width - (2 * inset)), bottom - top);

            var role = roles[note.Part];
            var colors = p.Part(parts[note.Part].ColorIndex);
            bool sounding = shift == TimeSpan.Zero && note.Start <= now && note.End > now;
            bool dimmed = dimBefore is { } dim && note.Start < dim;

            var fill = role == PartRole.AutoPlay
                ? (black ? colors.AutoDark : colors.Auto)
                : (black ? colors.Dark : colors.Normal);

            if (dimmed) dc.PushOpacity(0.3);
            double radius = Math.Min(3 * scale, bounds.Width / 2);
            dc.DrawRoundedRectangle(fill, sounding ? p.SoundingEdge : null, bounds, radius, radius);

            if (key != note.Note)
                DrawFoldMarker(dc, bounds, foldedDown: note.Note > key, radius);
            if (dimmed) dc.Pop();

            _drawnNotes.Add(new DrawnNote(note.Note, key, column, bounds));
        }
    }

    /// <summary>
    /// A note played an octave or more away from where it is written, because the keyboard does
    /// not reach it: a dashed outline, and a chevron pointing the way it really lies.
    /// </summary>
    private void DrawFoldMarker(DrawingContext dc, Rect bounds, bool foldedDown, double radius)
    {
        var p = _palette!;
        dc.DrawRoundedRectangle(null, p.FoldedEdge, bounds, radius, radius);

        double size = Math.Min(bounds.Width * 0.5, 7);
        if (size < 3 || bounds.Height < size + 4) return;

        double cx = bounds.X + (bounds.Width / 2);
        double tipY = foldedDown ? bounds.Bottom - size - 3 : bounds.Bottom - 3;
        double baseY = foldedDown ? bounds.Bottom - 3 : bounds.Bottom - size - 3;

        var chevron = new StreamGeometry();
        using (var g = chevron.Open())
        {
            // Up for a note that really sits above the keyboard, down for one below it.
            g.BeginFigure(new Point(cx - (size / 2), baseY), true, true);
            g.LineTo(new Point(cx, tipY), true, false);
            g.LineTo(new Point(cx + (size / 2), baseY), true, false);
        }
        chevron.Freeze();
        dc.DrawGeometry(p.FoldedMark, null, chevron);
    }

    private void DrawLoopMarkers(DrawingContext dc, TimeSpan loopStart, TimeSpan loopEnd,
                                 TimeSpan position, TimeSpan windowEnd, Func<TimeSpan, double> y,
                                 double laneLeft, double laneRight, double height)
    {
        var p = _palette!;

        // Shade the run-up before the loop starts, so it reads as not part of the loop.
        if (position < loopStart)
        {
            double startY = y(loopStart);
            dc.DrawRectangle(p.OutsideLoop, null, new Rect(laneLeft, Math.Max(0, startY), laneRight - laneLeft,
                                                            Math.Max(0, height - Math.Max(0, startY))));
        }

        foreach (var (mark, label) in new[] { (loopStart, "A"), (loopEnd, "B") })
        {
            if (mark < position || mark > windowEnd) continue;
            double lineY = Math.Round(y(mark)) + 0.5;
            dc.DrawLine(p.LoopEdge, new Point(laneLeft, lineY), new Point(laneRight, lineY));

            var text = new FormattedText(label, CultureInfo.InvariantCulture, FlowDirection.LeftToRight,
                                         LabelTypeface, 10, p.LoopText, VisualTreeHelper.GetDpi(this).PixelsPerDip);
            dc.DrawText(text, new Point(laneRight - text.Width - 4, lineY - text.Height - 1));
        }
    }

    private static int FirstStartingAtOrAfter(IReadOnlyList<SongNote> notes, TimeSpan time)
    {
        int lo = 0, hi = notes.Count;
        while (lo < hi)
        {
            int mid = (lo + hi) >>> 1;
            if (notes[mid].Start < time) lo = mid + 1;
            else hi = mid;
        }
        return lo;
    }

    /// <summary>Brushes and pens resolved once from the theme (Themes/Dark.xaml).</summary>
    private sealed class Palette
    {
        public Brush Lane { get; }
        public Pen OctaveLine { get; }
        public Pen GroupLine { get; }
        public Pen BarLine { get; }
        public Brush BarNumber { get; }
        public Brush HitLine { get; }
        public Pen LoopEdge { get; }
        public Brush LoopText { get; }
        public Brush OutsideLoop { get; }
        public Pen SoundingEdge { get; }
        public Pen FoldedEdge { get; }
        public Brush FoldedMark { get; }

        private readonly PartColors[] _parts;

        public Palette(FrameworkElement owner)
        {
            Brush Find(string key) => owner.TryFindResource(key) as Brush ?? Brushes.Magenta;
            Pen PenOf(string key, double thickness, DashStyle? dash = null) =>
                Frozen(new Pen(Find(key), thickness) { DashStyle = dash ?? DashStyles.Solid });

            Lane = Find("Brush.Song.Lane");
            OctaveLine = PenOf("Brush.Song.OctaveLine", 1);
            GroupLine = PenOf("Brush.Song.GroupLine", 1);
            BarLine = PenOf("Brush.Song.BarLine", 1);
            BarNumber = Find("Brush.Song.BarNumber");
            HitLine = Find("Brush.Song.HitLine");
            LoopEdge = PenOf("Brush.Song.Loop", 1);
            LoopText = Find("Brush.Song.Loop");
            OutsideLoop = Find("Brush.Song.OutsideLoop");
            SoundingEdge = PenOf("Brush.Song.Sounding", 1.5);
            FoldedEdge = PenOf("Brush.Song.Folded", 1, new DashStyle([2, 1.5], 0));
            FoldedMark = Find("Brush.Song.Folded");

            _parts = Enumerable.Range(0, PartColorCount)
                .Select(i => new PartColors(Find($"Brush.Song.Part{i}")))
                .ToArray();
        }

        public PartColors Part(int colorIndex) => _parts[((colorIndex % _parts.Length) + _parts.Length) % _parts.Length];

        private static Pen Frozen(Pen pen)
        {
            pen.Freeze();
            return pen;
        }
    }

    /// <summary>
    /// One part's fills. Black-key notes are a shade darker, echoing the keys; auto-play notes
    /// are translucent so the notes you play stand out.
    /// </summary>
    private sealed class PartColors
    {
        public Brush Normal { get; }
        public Brush Dark { get; }
        public Brush Auto { get; }
        public Brush AutoDark { get; }

        public PartColors(Brush themeBrush)
        {
            var color = themeBrush is SolidColorBrush solid ? solid.Color : Colors.Gray;
            var dark = Color.FromRgb((byte)(color.R * 0.72), (byte)(color.G * 0.72), (byte)(color.B * 0.72));

            Normal = Frozen(new SolidColorBrush(color));
            Dark = Frozen(new SolidColorBrush(dark));
            Auto = Frozen(new SolidColorBrush(color) { Opacity = 0.45 });
            AutoDark = Frozen(new SolidColorBrush(dark) { Opacity = 0.45 });
        }

        private static Brush Frozen(Brush brush)
        {
            brush.Freeze();
            return brush;
        }
    }
}
