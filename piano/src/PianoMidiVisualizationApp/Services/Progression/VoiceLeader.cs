using System.Numerics;

namespace PianoMidiVisualizationApp.Services.Progression;

/// <summary>
/// Voices a chord as close as possible to the one before it: every voice moves to a chord tone,
/// the voice count stays the same, and the total distance moved (in semitones) is minimised.
/// </summary>
/// <remarks>
/// This is plain nearest-tone voice leading, not part-writing. It does not forbid parallel
/// fifths or prefer root position, so C-E-G going to F yields C-F-A — an F/C, which is exactly
/// what a pianist's hand does.
/// </remarks>
public static class VoiceLeader
{
    /// <summary>Caps the search on a pathological input such as a forearm cluster.</summary>
    private const int MaxSearchSteps = 200_000;

    /// <param name="previous">The chord being left, as MIDI notes. Order is irrelevant.</param>
    /// <param name="pitchClasses">
    /// The target chord's pitch classes, most essential first (root, third, fifth, seventh).
    /// When there are fewer voices than pitch classes, the ones at the end are dropped.
    /// </param>
    /// <returns>The new chord's MIDI notes, ascending and distinct, one per voice.</returns>
    public static IReadOnlyList<int> Voice(IEnumerable<int> previous, IReadOnlyList<int> pitchClasses)
    {
        var voices = previous.Distinct().OrderBy(n => n).ToArray();
        var targets = pitchClasses.Select(MusicNaming.PitchClassOf).Distinct().ToArray();
        if (voices.Length == 0 || targets.Length == 0)
            return Array.Empty<int>();

        // Fewer voices than chord tones: keep the root and third and let the fifth go first.
        int requiredMask = 0;
        for (int i = 0; i < Math.Min(voices.Length, targets.Length); i++)
            requiredMask |= 1 << targets[i];

        // First let each voice take only the nearest instance of each chord tone above and
        // below it; anything further is never shorter. Only a dense cluster, with more voices
        // than there are distinct chord tones within reach, needs the wider second pass.
        for (int octaves = 1; octaves <= 2; octaves++)
        {
            var candidates = voices.Select(v => CandidatesFor(v, targets, octaves)).ToArray();
            if (candidates.SelectMany(c => c).Distinct().Count() < voices.Length)
                continue;   // not enough distinct keys in reach to give every voice its own

            var search = new Search(voices, candidates, requiredMask);
            search.Run();
            if (search.Result is { } result)
                return result;
        }

        return Stacked(voices, targets);
    }

    private static int[] CandidatesFor(int voice, int[] targets, int octaves)
    {
        var list = new List<int>(targets.Length * octaves * 2);
        foreach (int pc in targets)
        {
            int up = voice + MusicNaming.PitchClassOf(pc - voice);
            for (int o = 0; o < octaves; o++)
            {
                int above = up + (o * 12);
                int below = up - ((o + 1) * 12);
                if (above <= 127) list.Add(above);
                if (below >= 0 && below != voice) list.Add(below);
            }
        }
        return list.Distinct().OrderBy(n => Math.Abs(n - voice)).ThenBy(n => n).ToArray();
    }

    /// <summary>
    /// Depth-first over voices with branch-and-bound on the running cost. The candidate lists
    /// are sorted nearest-first, so the first complete solution is already a tight bound.
    /// </summary>
    private sealed class Search(int[] voices, int[][] candidates, int requiredMask)
    {
        private readonly int[] _current = new int[voices.Length];
        private readonly HashSet<int> _used = new();

        /// <summary>The least the voices from each index onward could possibly move.</summary>
        private readonly int[] _floor = SuffixFloor(voices, candidates);

        private (int Total, int Max, int Crossings) _bestScore = (int.MaxValue, int.MaxValue, int.MaxValue);
        private int _steps;

        public int[]? Result { get; private set; }

        public void Run() => Step(0, 0, 0);

        private void Step(int index, int cost, int coveredMask)
        {
            // Past the budget, keep whatever has been found; a real hand never gets near it.
            if (++_steps > MaxSearchSteps) return;
            if (cost + _floor[index] > _bestScore.Total) return;

            // Not enough voices left to pick up the chord tones still missing.
            if (BitOperations.PopCount((uint)(requiredMask & ~coveredMask)) > voices.Length - index) return;

            if (index == voices.Length)
            {
                var score = Score(cost);
                if (score.CompareTo(_bestScore) < 0)
                {
                    _bestScore = score;
                    Result = _current.OrderBy(n => n).ToArray();
                }
                return;
            }

            foreach (int note in candidates[index])
            {
                // Two voices on one key is one voice: the keyboard can't sound a note twice.
                if (!_used.Add(note)) continue;
                _current[index] = note;
                Step(index + 1, cost + Math.Abs(note - voices[index]),
                     coveredMask | (1 << MusicNaming.PitchClassOf(note)));
                _used.Remove(note);
            }
        }

        private static int[] SuffixFloor(int[] voices, int[][] candidates)
        {
            var floor = new int[voices.Length + 1];
            for (int i = voices.Length - 1; i >= 0; i--)
                floor[i] = floor[i + 1] + (candidates[i].Length > 0 ? Math.Abs(candidates[i][0] - voices[i]) : 0);
            return floor;
        }

        /// <summary>
        /// Least total motion first; then no single voice leaping further than it must; then
        /// voices keeping their order, so the bass stays the bass.
        /// </summary>
        private (int Total, int Max, int Crossings) Score(int total)
        {
            int max = 0, crossings = 0;
            for (int i = 0; i < voices.Length; i++)
            {
                max = Math.Max(max, Math.Abs(_current[i] - voices[i]));
                for (int j = i + 1; j < voices.Length; j++)
                    if (_current[j] < _current[i]) crossings++;
            }
            return (total, max, crossings);
        }
    }

    /// <summary>
    /// Last resort: chord tones stacked upward from the bass, one per voice. Only reached when
    /// there are too many voices for distinct chord tones within two octaves of each.
    /// </summary>
    private static int[] Stacked(int[] voices, int[] targets)
    {
        var result = new List<int>(voices.Length);
        var ascending = targets.OrderBy(pc => MusicNaming.PitchClassOf(pc - targets[0])).ToArray();
        int note = voices[0] - MusicNaming.PitchClassOf(voices[0] - targets[0]);
        for (int i = 0; result.Count < voices.Length && note <= 127; i++)
        {
            int pc = ascending[i % ascending.Length];
            int octave = i / ascending.Length;
            int candidate = note + (octave * 12) + MusicNaming.PitchClassOf(pc - targets[0]);
            if (candidate is >= 0 and <= 127) result.Add(candidate);
        }
        return result.ToArray();
    }
}
