//! The standalone program's icon (`assets/amp.ico`, made by `scripts/make-icon.py`): every size
//! the taskbar, the title bar and Explorer ask for, each drawn for its size with a transparent
//! surround, and the large ones sharp rather than a small picture scaled up

use std::path::PathBuf;

const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

struct Image {
    size: usize,
    rgba: Vec<u8>,
}

/// The images of an .ico file whose entries are PNGs, as the script writes it
fn read_ico() -> Vec<Image> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/amp.ico");
    let bytes = std::fs::read(&path).unwrap();
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    assert_eq!((u16_at(0), u16_at(2)), (0, 1), "not an icon file");
    (0..u16_at(4))
        .map(|i| {
            let entry = 6 + 16 * i;
            let (len, offset) = (u32_at(entry + 8), u32_at(entry + 12));
            let mut decoder = png::Decoder::new(&bytes[offset..offset + len]);
            decoder.set_transformations(png::Transformations::EXPAND);
            let mut reader = decoder.read_info().unwrap();
            let mut rgba = vec![0; reader.output_buffer_size()];
            let info = reader.next_frame(&mut rgba).unwrap();
            assert_eq!(
                (info.color_type, info.bit_depth),
                (png::ColorType::Rgba, png::BitDepth::Eight),
                "entry {i} is not RGBA"
            );
            assert_eq!(info.width, info.height);
            Image { size: info.width as usize, rgba }
        })
        .collect()
}

#[test]
fn test_the_icon_has_every_size_with_a_transparent_surround() {
    let images = read_ico();
    let mut sizes: Vec<u32> = images.iter().map(|i| i.size as u32).collect();
    sizes.sort_unstable();
    assert_eq!(sizes, SIZES, "amp.ico sizes (run scripts/make-icon.py)");
    for image in &images {
        let n = image.size;
        let alpha = |x: usize, y: usize| image.rgba[(y * n + x) * 4 + 3];
        let corners = [alpha(0, 0), alpha(n - 1, 0), alpha(0, n - 1), alpha(n - 1, n - 1)];
        assert!(
            corners.iter().all(|&a| a == 0),
            "{n} px: the corners are not fully transparent: {corners:?}"
        );
        assert!((0..n * n).any(|i| image.rgba[i * 4 + 3] == 255), "{n} px: nothing is opaque");
    }
}

/// The share of an image's edges that are hard: of the steps between neighbouring pixels (across
/// and down) that change a channel by 10 or more, the part that changes one by 120 or more. An
/// edge drawn at the image's size goes from one ink to the next within a pixel or two; a smaller
/// picture scaled up spreads the same edge over a few more pixels, and the share falls
fn hard_edge_share(image: &Image) -> f64 {
    let n = image.size;
    let at = |x: usize, y: usize, c: usize| image.rgba[(y * n + x) * 4 + c] as i32;
    let (mut hard, mut soft) = (0u32, 0u32);
    for y in 0..n - 1 {
        for x in 0..n - 1 {
            for (x2, y2) in [(x + 1, y), (x, y + 1)] {
                let step = (0..4).map(|c| (at(x, y, c) - at(x2, y2, c)).abs()).max().unwrap();
                match step {
                    120.. => hard += 1,
                    10.. => soft += 1,
                    _ => {}
                }
            }
        }
    }
    hard as f64 / (hard + soft).max(1) as f64
}

#[test]
fn test_the_large_icon_is_drawn_sharp_not_scaled_up() {
    let images = read_ico();
    let big = images.iter().find(|i| i.size == 256).unwrap();
    // Measured: 0.17 for the drawn icon, 0.09 for the earlier one, scaled up from 180 px
    let share = hard_edge_share(big);
    assert!(
        share > 0.13,
        "only {share:.2} of the 256 px icon's edges are hard: it looks scaled up from a smaller picture \
         (run scripts/make-icon.py)"
    );
}

#[test]
fn test_what_counts_as_scaled_up() {
    // A one-pixel edge is hard, the same edge spread over eight pixels is not
    let n = 32;
    let make = |f: &dyn Fn(usize) -> u8| Image {
        size: n,
        rgba: (0..n * n).flat_map(|i| [f(i % n), 0, 0, 255]).collect(),
    };
    let sharp = make(&|x| if x < n / 2 { 0 } else { 255 });
    let ramp = make(&|x| ((x.clamp(12, 20) - 12) * 255 / 8) as u8);
    assert!(hard_edge_share(&sharp) > 0.9);
    assert!(hard_edge_share(&ramp) < 0.1);
}
