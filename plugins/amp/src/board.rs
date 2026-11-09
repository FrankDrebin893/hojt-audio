//! The signal line under the strip: every block of the chain at full size in the order the
//! signal goes through them (the gate, the effects in front of the amps, the stack of the amp
//! lanes with their cabinets, the effects behind them), with couplers between the blocks that
//! stand side by side and patch cables into the head and out of the cabinet. A block is moved by
//! dragging it by its title: the others make room, and it lands in the nearest place it may
//! have.
//!
//! The line is the chain model's (`slots::Chain`, kept with the parameters and saved with the
//! project) and its width follows it: a block keeps its size and the gap between two blocks is
//! always the same, so a longer chain is a wider line, which the window scales or scrolls
//! (`editor::Fit`). What lives in the window is only where the blocks are drawn while one is
//! held or gliding to its place.

use nih_plug_egui::egui::{vec2, Id, Painter, Pos2, Rect, Ui};
use suite_common::ui::{self, BENCH_MARGIN, PLUG_Y};

use crate::editor::{Jacks, Pedals, PEDAL_HEIGHT, PEDAL_TOP, PEDAL_WIDTH, STACK_TOP};
use crate::order::Zone;
use crate::slots::{Block, Chain, StoredChain, MAX_SLOTS};

/// The two lanes (a head and its cabinet each) and the plate where they meet
pub const STACK_WIDTH: f32 = 700.0;
/// The space between two neighbours of the line. What it always was: the old row of thirteen
/// pedals and the stack filled 3411 points with it
pub const GAP: f32 = 27.0;

// Share of its way to its place that a block making room goes in a second, were it to keep its speed
const GLIDE_PER_SECOND: f32 = 18.0;

/// One piece of the line: the gate that is always first, a block of the chain, or the stack of
/// amp lanes that the blocks in front of the amps lead into
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Piece {
    Gate,
    Block(Block),
    Stack,
}

/// What a piece is called while it is drawn and glides: the gate, the stack and one for each
/// slot, so a block that moves is the same piece after it
const KEYS: usize = 2 + MAX_SLOTS;

impl Piece {
    fn key(self) -> usize {
        match self {
            Piece::Gate => 0,
            Piece::Stack => 1,
            Piece::Block(block) => 2 + block.slot as usize,
        }
    }

    pub(crate) fn width(self) -> f32 {
        match self {
            Piece::Stack => STACK_WIDTH,
            Piece::Gate | Piece::Block(_) => PEDAL_WIDTH,
        }
    }

    /// Only a block of the chain is moved: the gate is always first, and the stack is where the
    /// amps are
    fn movable(self) -> bool {
        matches!(self, Piece::Block(_))
    }
}

/// The pieces in signal order. The stack is `stack`th of them: the gate and the blocks in front
/// of the amps come before it, the blocks behind the cabinets after it
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Line {
    pieces: Vec<Piece>,
    stack: usize,
}

impl Line {
    /// The line a chain makes
    pub(crate) fn of_chain(chain: &Chain) -> Line {
        let mut pieces = vec![Piece::Gate];
        pieces.extend(chain.front().iter().map(|&block| Piece::Block(block)));
        let stack = pieces.len();
        pieces.push(Piece::Stack);
        pieces.extend(chain.back().iter().map(|&block| Piece::Block(block)));
        Line { pieces, stack }
    }

    /// The width of the line's design layer: a margin at each end, every piece and a gap
    /// between each two
    pub(crate) fn width(&self) -> f32 {
        let pieces: f32 = self.pieces.iter().map(|piece| piece.width()).sum();
        2.0 * BENCH_MARGIN + pieces + GAP * (self.pieces.len() - 1) as f32
    }

    #[cfg(test)]
    pub(crate) fn pieces_for_test(&self) -> &[Piece] {
        &self.pieces
    }

    /// The left edge of the piece at each place of the line, from the left edge of the layer
    #[cfg(test)]
    pub(crate) fn places_for_test(&self) -> Vec<f32> {
        let lefts = self.places();
        self.pieces.iter().map(|piece| lefts[piece.key()]).collect()
    }

    /// The left edge of every piece (by `Piece::key`), from the left edge of the layer
    fn places(&self) -> [f32; KEYS] {
        let mut lefts = [0.0; KEYS];
        let mut x = BENCH_MARGIN;
        for piece in &self.pieces {
            lefts[piece.key()] = x;
            x += piece.width() + GAP;
        }
        lefts
    }

    /// Where `piece` stands as the chain counts it: its zone and its number in it
    fn zone_and_place(&self, piece: Piece) -> Option<(Zone, usize)> {
        let at = self.pieces.iter().position(|&other| other == piece)?;
        Some(if at < self.stack {
            (Zone::Front, at - 1)
        } else {
            (Zone::Back, at - self.stack - 1)
        })
    }

    /// Every line with `piece` in a place it may have, this one included, and the others as
    /// they follow each other now. None for a piece that is fixed
    fn moves(&self, piece: Piece) -> Vec<Line> {
        let (Piece::Block(block), Some(was)) =
            (piece, self.pieces.iter().position(|&other| other == piece))
        else {
            return Vec::new();
        };
        let mut others = self.pieces.clone();
        others.remove(was);
        // The stack is at this place among the others
        let stack = self.stack - usize::from(was < self.stack);

        let mut found = Vec::new();
        // Never in front of the gate
        for at in 1..=others.len() {
            let in_front = at <= stack;
            let zone = if in_front { Zone::Front } else { Zone::Back };
            if block.kind.can_be_in(zone) {
                let mut pieces = others.clone();
                pieces.insert(at, piece);
                found.push(Line { pieces, stack: stack + usize::from(in_front) });
            }
        }
        found
    }

    /// The line `piece` would land in if it were let go with its left edge at `left`: of the
    /// places it may have, the one nearest to there
    fn nearest(&self, piece: Piece, left: f32) -> Line {
        let distance = |line: &Line| (line.places()[piece.key()] - left).abs();
        self.moves(piece)
            .into_iter()
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
            .unwrap_or_else(|| self.clone())
    }
}

/// The piece that is held by its title
#[derive(Clone, Copy)]
struct Held {
    piece: Piece,
    /// From the piece's left edge to where the pointer took it
    grip: f32,
    /// Its left edge, from the left edge of the layer
    left: f32,
}

/// What the window keeps of the line from frame to frame
#[derive(Clone)]
struct Board {
    held: Option<Held>,
    /// Where every piece is drawn now: they glide to their places
    lefts: [f32; KEYS],
    /// Which pieces were in the line the last time it was drawn: one that is new stands where it
    /// belongs at once
    present: [bool; KEYS],
}

/// The whole line under the strip, in the order `chain` holds, which a block that is dragged and
/// let go changes. `window` is the layer's top left corner (the line's own, scrolled);
/// `pedals` draws each piece
pub fn board(ui: &mut Ui, window: Pos2, pedals: &Pedals, chain: &StoredChain) {
    // Read every frame: a preset or the host may have changed it
    let line = Line::of_chain(&chain.get());
    let id = Id::new("board");
    let mut board: Board = ui.data(|data| data.get_temp(id)).unwrap_or(Board {
        held: None,
        lefts: line.places(),
        present: [false; KEYS],
    });
    let held = board.held.map(|held| held.piece);
    // A block that is held and was taken out of the chain meanwhile (a preset was loaded) is let go
    if held.is_some_and(|piece| !line.pieces.contains(&piece)) {
        board.held = None;
    }
    let held = board.held.map(|held| held.piece);

    // The others stand where they would if the held piece were let go now
    let shown = board.held.map_or_else(|| line.clone(), |held| line.nearest(held.piece, held.left));
    let places = shown.places();
    let step = (ui.input(|input| input.stable_dt) * GLIDE_PER_SECOND).min(1.0);
    let mut gliding = false;
    let mut present = [false; KEYS];
    for &piece in &shown.pieces {
        let key = piece.key();
        present[key] = true;
        if Some(piece) == held {
            continue;
        }
        let (at, to) = (&mut board.lefts[key], places[key]);
        if !board.present[key] || (to - *at).abs() < 0.5 {
            *at = to;
        } else {
            *at += (to - *at) * step;
            gliding = true;
        }
    }
    board.present = present;
    if gliding {
        ui.ctx().request_repaint();
    }

    // The stack first, so that a piece carried across it is in front of it. It keeps its
    // size while it glides, so nothing of it is drawn anew
    let jacks = pedals.stack(ui, window + vec2(board.lefts[Piece::Stack.key()], STACK_TOP));

    // The held piece last, so it is above the others and the cables
    let in_turn = shown
        .pieces
        .iter()
        .copied()
        .filter(|&piece| piece != Piece::Stack && Some(piece) != held)
        .chain(held);
    let mut cabled = false;
    for piece in in_turn {
        if Some(piece) == held {
            signal(ui.painter(), window, &shown, &board.lefts, held, jacks);
            cabled = true;
        }
        let left = board.lefts[piece.key()];
        let mut rect =
            Rect::from_min_size(window + vec2(left, PEDAL_TOP), vec2(piece.width(), PEDAL_HEIGHT));
        if Some(piece) == held {
            rect = ui::lifted(ui.painter(), rect);
        }
        let (face, grip_id) = match piece {
            Piece::Gate => (pedals.gate(ui, rect), None),
            Piece::Block(block) => {
                (pedals.block(ui, block, rect), Some(format!("block-{}", block.slot)))
            }
            Piece::Stack => continue,
        };
        let Some(grip_id) = grip_id.filter(|_| piece.movable()) else {
            continue;
        };

        let grip = ui::title_grip(ui, face, &grip_id);
        if grip.drag_started() {
            // Where the button went down, not where the drag was noticed
            let from_global =
                ui.ctx().layer_transform_from_global(ui.layer_id()).unwrap_or_default();
            let taken = ui
                .input(|input| input.pointer.press_origin())
                .map_or(face.center().x, |pos| (from_global * pos).x);
            board.held = Some(Held { piece, grip: taken - rect.left(), left });
        }
        if let (Some(held), Some(pointer)) = (&mut board.held, grip.interact_pointer_pos()) {
            // Not over the gate and not out of the line
            let leftmost = BENCH_MARGIN + PEDAL_WIDTH + GAP;
            let rightmost = line.width() - BENCH_MARGIN - piece.width();
            held.left = (pointer.x - window.x - held.grip).clamp(leftmost, rightmost);
            board.lefts[piece.key()] = held.left;
        }
        if grip.drag_stopped() {
            if let (Some(held), Piece::Block(block)) = (board.held.take(), piece) {
                let landed = line.nearest(held.piece, held.left);
                if let Some((zone, place)) = landed.zone_and_place(held.piece) {
                    let mut moved = chain.get();
                    if moved.move_block(block.slot, zone, place).is_ok() && moved != chain.get() {
                        chain.put(moved);
                    }
                }
            }
        }
    }
    if !cabled {
        signal(ui.painter(), window, &shown, &board.lefts, held, jacks);
    }

    ui.data_mut(|data| data.insert_temp(id, board));
}

/// The signal from piece to piece of `shown`, where they are drawn now (`lefts`): a coupler
/// between two pieces that stand side by side, a patch cable from the last piece in front
/// of the amps into the head and one from the cabinet to the first piece behind it. The
/// piece that is held has no couplers, but the cable to it goes where it is carried
fn signal(
    painter: &Painter,
    window: Pos2,
    shown: &Line,
    lefts: &[f32; KEYS],
    held: Option<Piece>,
    jacks: Jacks,
) {
    let plug_y = window.y + PEDAL_TOP + PLUG_Y;
    let left_of = |piece: Piece| window.x + lefts[piece.key()];
    let right_of = |piece: Piece| left_of(piece) + piece.width();
    for pair in shown.pieces.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if a == Piece::Stack || b == Piece::Stack || held == Some(a) || held == Some(b) {
            continue;
        }
        // Not while one of them glides past: only once they stand side by side
        let (from, to) = (right_of(a), left_of(b));
        if (to - from - GAP).abs() < 1.0 {
            ui::coupler(painter, from, to, plug_y);
        }
    }
    let before = shown.pieces[shown.stack - 1];
    ui::patch_cable(painter, jacks.input, Pos2::new(right_of(before), plug_y), 1.0);
    if let Some(&after) = shown.pieces.get(shown.stack + 1) {
        ui::patch_cable(painter, jacks.output, Pos2::new(left_of(after), plug_y), -1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::{ChainOrder, Pedal};
    use crate::slots::EffectKind;

    fn default_line() -> Line {
        Line::of_chain(&Chain::default())
    }

    fn blocks(line: &Line) -> Vec<Block> {
        line.pieces
            .iter()
            .filter_map(|piece| if let Piece::Block(block) = piece { Some(*block) } else { None })
            .collect()
    }

    /// Every line that is `depth` moves away from `line`
    fn lines_after(line: &Line, depth: usize) -> Vec<Line> {
        let mut lines = vec![line.clone()];
        for _ in 0..depth {
            lines = lines
                .iter()
                .flat_map(|line| {
                    blocks(line).into_iter().flat_map(|block| line.moves(Piece::Block(block)))
                })
                .collect();
        }
        lines
    }

    /// A chain of sixteen blocks, as many in front of the amps as behind
    fn full_chain() -> Chain {
        let mut chain = Chain::empty();
        for number in 0..16 {
            let (kind, zone) = if number % 2 == 0 {
                (EffectKind::Comp, Zone::Front)
            } else {
                (EffectKind::Delay, Zone::Back)
            };
            chain.add(kind, zone, 99).unwrap();
        }
        chain
    }

    #[test]
    fn test_the_line_of_a_project_from_before_is_the_old_board_to_the_point() {
        // The gate, the twelve pedals and the stack filled 3411 points, a gap of 27 between each
        let line = default_line();
        assert_eq!(line.pieces.len(), 14);
        assert_eq!(line.width(), 3411.0);
        let lefts = line.places();
        assert_eq!(lefts[Piece::Gate.key()], BENCH_MARGIN);
        for pair in line.pieces.windows(2) {
            assert_eq!(lefts[pair[0].key()] + pair[0].width() + GAP, lefts[pair[1].key()]);
        }
        let last = *line.pieces.last().unwrap();
        assert_eq!(lefts[last.key()] + last.width() + BENCH_MARGIN, line.width());
        // In the order the chain had: the gate, six in front, the stack, six behind
        assert_eq!(line.pieces[0], Piece::Gate);
        assert_eq!(line.pieces[line.stack], Piece::Stack);
        assert_eq!(line.stack, 7);
        // Every block is as wide as the next
        assert!(blocks(&line).iter().all(|&block| Piece::Block(block).width() == PEDAL_WIDTH));
    }

    #[test]
    fn test_the_line_follows_the_chain_and_a_block_keeps_its_size() {
        let empty = Line::of_chain(&Chain::empty());
        assert_eq!(empty.pieces, [Piece::Gate, Piece::Stack]);
        assert_eq!(empty.width(), 2.0 * BENCH_MARGIN + PEDAL_WIDTH + GAP + STACK_WIDTH);
        // Each block adds one block and one gap, whichever side it stands on
        let full = Line::of_chain(&full_chain());
        assert_eq!(full.pieces.len(), 2 + 16);
        assert_eq!(full.width(), empty.width() + 16.0 * (PEDAL_WIDTH + GAP));
        assert_eq!(full.stack, 1 + 8);
        // Sixteen blocks on sixteen slots: each has a place of its own to glide from
        let mut keys: Vec<usize> = full.pieces.iter().map(|piece| piece.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), full.pieces.len());
        assert!(keys.iter().all(|&key| key < KEYS));
    }

    #[test]
    fn test_no_move_breaks_the_rules() {
        // Every line that is two moves away from the default one
        let lines = lines_after(&default_line(), 2);
        assert!(lines.len() > 1000);
        for line in lines {
            assert_eq!(line.pieces[0], Piece::Gate, "{line:?}");
            assert_eq!(line.pieces[line.stack], Piece::Stack, "{line:?}");
            assert_eq!(line.pieces.iter().filter(|&&piece| piece == Piece::Stack).count(), 1);
            let mut slots: Vec<u8> = blocks(&line).iter().map(|block| block.slot).collect();
            slots.sort_unstable();
            assert_eq!(slots, (0..12).collect::<Vec<u8>>(), "{line:?}");
        }
    }

    #[test]
    fn test_the_line_is_the_chain_and_back_whatever_is_moved() {
        // Dropping a block where a move puts it makes the chain the line shows, slots and all
        let chain = Chain::default();
        for block in blocks(&default_line()) {
            for line in default_line().moves(Piece::Block(block)) {
                let (zone, place) = line.zone_and_place(Piece::Block(block)).unwrap();
                let mut moved = chain;
                assert!(moved.move_block(block.slot, zone, place).is_ok());
                assert_eq!(Line::of_chain(&moved), line);
            }
        }
        // And a second move from there, with the twelve-pedal view of the chain still legal
        for first in lines_after(&default_line(), 1).into_iter().step_by(7) {
            let mut chain_one = chain;
            if let Some(block) = blocks(&first).into_iter().next() {
                let (zone, place) = first.zone_and_place(Piece::Block(block)).unwrap();
                chain_one.move_block(block.slot, zone, place).unwrap();
            }
            for second in lines_after(&Line::of_chain(&chain_one), 1).into_iter().step_by(11) {
                let mut moved = chain_one;
                for block in blocks(&second) {
                    let (zone, place) = second.zone_and_place(Piece::Block(block)).unwrap();
                    if Line::of_chain(&moved).zone_and_place(Piece::Block(block))
                        != Some((zone, place))
                    {
                        moved.move_block(block.slot, zone, place).unwrap();
                        break;
                    }
                }
                let order = moved.order();
                assert_eq!(order.front().len() + order.back().len(), Pedal::ALL.len());
            }
        }
        assert_eq!(Chain::from_order(&ChainOrder::default()), Chain::default());
    }

    #[test]
    fn test_a_second_block_of_a_kind_is_a_piece_of_its_own_and_moves_with_its_slot() {
        let mut chain = Chain::default();
        let second = chain.add(EffectKind::Delay, Zone::Front, 0).unwrap();
        assert_ne!(second as usize, Pedal::Delay.index());
        let line = Line::of_chain(&chain);
        let own = Piece::Block(chain.block_on(second).unwrap());
        assert_eq!(line.pieces[1], own);
        assert_eq!(line.pieces.len(), 15);
        // Carried across the amps it lands behind them, and is still on its slot
        let landed = line.nearest(own, line.width());
        assert_eq!(*landed.pieces.last().unwrap(), own);
        let (zone, place) = landed.zone_and_place(own).unwrap();
        assert_eq!(zone, Zone::Back);
        let mut moved = chain;
        moved.move_block(second, zone, place).unwrap();
        assert_eq!(moved.back().last().map(|block| block.slot), Some(second));
    }

    #[test]
    fn test_fixed_pieces_have_no_moves() {
        let line = default_line();
        assert!(line.moves(Piece::Gate).is_empty());
        assert!(line.moves(Piece::Stack).is_empty());
        assert_eq!(line.nearest(Piece::Stack, 0.0), line);
        assert!(!Piece::Gate.movable() && !Piece::Stack.movable());
    }

    #[test]
    fn test_a_gate_block_stays_in_front_of_the_amps() {
        let mut chain = Chain::default();
        let slot = chain.add(EffectKind::Gate, Zone::Front, 0).unwrap();
        let line = Line::of_chain(&chain);
        let gate = Piece::Block(chain.block_on(slot).unwrap());
        for moved in line.moves(gate) {
            assert!(moved.pieces.iter().position(|&p| p == gate) < Some(moved.stack));
        }
        // As far right as it goes it is last in front, not behind the cabinet
        let landed = line.nearest(gate, line.width());
        assert_eq!(landed.pieces[landed.stack - 1], gate);
    }

    fn pedal_block(pedal: Pedal) -> Piece {
        Piece::Block(Block { slot: pedal.index() as u8, kind: EffectKind::of_pedal(pedal) })
    }

    #[test]
    fn test_a_block_lands_in_the_nearest_place_it_may_have() {
        let line = default_line();
        let places = line.places();
        // Let go where it stands, nothing moves
        for block in blocks(&line) {
            let piece = Piece::Block(block);
            assert_eq!(line.nearest(piece, places[piece.key()]), line);
        }
        let landing = |pedal: Pedal, left: f32| {
            let landed = line.nearest(pedal_block(pedal), left);
            (landed.zone_and_place(pedal_block(pedal)).unwrap(), landed.stack)
        };
        // The chorus across the amp to right behind the gate
        assert_eq!(landing(Pedal::Chorus, 0.0), ((Zone::Front, 0), 8));
        // The drive as far right as it goes: last behind the cabinet (#268)
        assert_eq!(landing(Pedal::Drive, line.width()), ((Zone::Back, 6), 6));
        // The reverb as far left as it goes: right behind the gate, in front of the amp
        assert_eq!(landing(Pedal::Reverb, 0.0), ((Zone::Front, 0), 8));
        // Just behind the cabinet is still possible
        assert_eq!(
            landing(Pedal::Reverb, places[pedal_block(Pedal::Chorus).key()]).0 .0,
            Zone::Back
        );
        // Right in front of the amp: the last place before it
        let landed = line.nearest(pedal_block(Pedal::Eq), places[Piece::Stack.key()]);
        assert_eq!(landed.pieces[landed.stack - 1], pedal_block(Pedal::Eq));
        // The wah as far left as it goes is first after the gate
        let landed = line.nearest(pedal_block(Pedal::Wah), 0.0);
        assert_eq!(landed.pieces[1], pedal_block(Pedal::Wah));
    }
}
