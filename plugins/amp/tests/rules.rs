//! Rules of the Guitar Amp in plugins/README.md that a test can hold: it is all our own designs, with
//! no real makers' names, trademarks or circuit names anywhere, and the amps differ only by
//! the constants in `amp/model.rs`

use std::path::{Path, PathBuf};

/// Real makers' names, trademarks and circuit names, one word or two in a row, lowercase, by
/// their FNV-1a hash: written out, the list would break the rule it holds. To add one, run
/// `NAME="the name" cargo test -p amp --test rules -- --ignored --nocapture hash_of_name`
/// and add the line it prints
const NAMES: &[u64] = &[
    0x0102cebcc7e25b95,
    0x063d5d02a4be2b41,
    0x063e0383f4330183,
    0x06449283f438602e,
    0x07ba45191724cb96,
    0x08ebfbd72f8c640b,
    0x0d0f06ccf584e322,
    0x0f750c669c858eee,
    0x116cf6575615ca98,
    0x13cd9bf65f93660f,
    0x143a6de9a1b2a7c4,
    0x1b8f7bcd401a1275,
    0x201eecccaf3decb2,
    0x21a29e4e2b615c57,
    0x2592f0292e0c1420,
    0x267f3cc6f88f1efb,
    0x2869c49dc6600056,
    0x2917926c390ffc2f,
    0x29a95bc44a0ca7c7,
    0x2d1cc85a3f65032f,
    0x31038408d1f51041,
    0x3546804c852a321e,
    0x390f94044b08a073,
    0x3b433408c57fdac0,
    0x3b4b8cc9a2ae1250,
    0x3be079b66f7f37f0,
    0x3e28de757da0b6f4,
    0x3ed937cd87818fe5,
    0x3fb7da00d69d9f8c,
    0x40592f128f560e73,
    0x432411a2e3312607,
    0x454463193a423852,
    0x454a7f193a46d394,
    0x455b62193a551580,
    0x456c7d193a63b694,
    0x458b83193a7e7155,
    0x48b1c28c92c8fda9,
    0x4cc05f1fbfbd1b2c,
    0x4f4abd4f0079b22e,
    0x5198bfef4b75998b,
    0x533d6973f00b9472,
    0x545eb6828da59057,
    0x560b650332571d68,
    0x5706fd194470a10b,
    0x575548f833875e0d,
    0x5851c6ef65d3e356,
    0x5991dd11f4002697,
    0x5a7fa9d74753eab1,
    0x5b0fb8e0bbbf552b,
    0x5ea52f2f9225a10f,
    0x6382f9af54ee5601,
    0x63fdc4ac3ac22ccf,
    0x64ac340c6dd70b46,
    0x6840dede44e12d18,
    0x6867e2194e4cbeda,
    0x68ec50194ebd2a85,
    0x68fd12194ecb345e,
    0x6a43ab98bc900b17,
    0x75eededf231a3a84,
    0x7738b114ad15454f,
    0x79e4a170ea19aed5,
    0x7e8d359b99f4e9c1,
    0x80120f6d842b2582,
    0x8146e7ea2e4902b2,
    0x851fbb36f5a69296,
    0x89a3ca9fcff802c3,
    0x8a6ae1961e6520dd,
    0x8ae66085099598a4,
    0x8c2d47b62911316c,
    0x8cff1d465a0757d2,
    0x8db06b377d699e05,
    0x8dbf050c9e698076,
    0x9121cb605d8dee41,
    0x91bc1cade58afc0d,
    0x92066dac3e5d3561,
    0x925f6fc18ffd9e04,
    0x929e2863210065ad,
    0x95dea6cfa44b9ae6,
    0x96507dbdfd4388a7,
    0x9831faf30dc646f8,
    0x98be908c0ce9e833,
    0x99c28ac193d036ef,
    0x9d023b4868d2acae,
    0xa1af5927aa2677e6,
    0xa282fe28c45094da,
    0xa2e193278b57ad00,
    0xa45f0739235b3189,
    0xa86af8da6fc0e9d6,
    0xaa060950166d70e2,
    0xab5bd7c5ec112a69,
    0xad7d7fd2b0fb4c06,
    0xb169f40ae8d697e1,
    0xb24c38b986cd32ac,
    0xb3e131d3a5ee2d29,
    0xb55f312dd581cd4b,
    0xb6e5791921d9e76f,
    0xb87413c3764a84e4,
    0xc019b09cb6208f32,
    0xc063e31993f49b43,
    0xc2db0318f040f400,
    0xc30af718f06a035e,
    0xc46ae461929e8b5c,
    0xc58699ece4dce7a3,
    0xc6a5ebea87b4429b,
    0xc6c3366c06ff0789,
    0xc89695ada1892de7,
    0xc9669845e9b2e07b,
    0xca90d54b73a665e6,
    0xd14823ec544a2582,
    0xd44d1918fa2bd845,
    0xd743a8f61caa2669,
    0xd796889d12326080,
    0xd815cfe5d2f283fd,
    0xd8fb90eb1f22be15,
    0xdcab8c18fed3ddde,
    0xdde4e63479d6d9a9,
    0xdee188d27109bbef,
    0xe5c045b80d74bd1f,
    0xe74e54aff767cca7,
    0xe80ee36fd061e0a7,
    0xe8dab766d7d398a3,
    0xea69ac6f5f2d4ce2,
    0xead9cd7118804bf8,
    0xead9d27118805477,
    0xead9d67118805b43,
    0xeb2231c20515c2e9,
    0xebd6d4830a92a6ad,
    0xed7898936bf5a019,
    0xee05af347b5dd3e1,
    0xef218043093e2dd2,
    0xf0aaab66dc02c129,
    0xf28ae7dab62883d5,
    0xf319cdad181db12d,
    0xf40ba71bbf138892,
    0xf4a3d3ee54f1ca9d,
    0xf5b078eb70d4369d,
    0xf680b83414c33f09,
    0xf7c79180e91a8bce,
    0xf914b1aee0b74231,
    0xf97987246db48bca,
    0xf9b4c1151a1c16f0,
    0xf9c40d15cb825eb9,
    0xfa8f4b8fad7528d2,
];

#[test]
fn test_the_amp_names_no_maker() {
    let mut found = Vec::new();
    for path in files(&amp_dir(), &|_| true) {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        for (line, name) in named(&text, NAMES) {
            found.push(format!("{}:{line}: \"{name}\"", shown(&path)));
        }
    }
    assert!(
        found.is_empty(),
        "real makers' names, trademarks or circuit names in the amp:\n{}\n\nThe Guitar Amp is all \
         our own designs, with our own names (Guitar Amp in plugins/README.md): say what the part does, \
         or give it a name of our own",
        found.join("\n")
    );
}

#[test]
fn test_no_dsp_branches_on_the_amp() {
    // Picking an amp is the model's job; the editor and the looks show each amp its own way
    // `test_util.rs` is a module of tests only
    let allowed = ["amp/model.rs", "editor.rs", "looks.rs", "test_util.rs"];
    let src = amp_dir().join("src");
    let is_dsp = |path: &Path| {
        path.extension().is_some_and(|ext| ext == "rs")
            && !allowed.iter().any(|allowed| path.ends_with(allowed))
    };
    let mut found = Vec::new();
    for path in files(&src, &is_dsp) {
        let text = std::fs::read_to_string(&path).unwrap();
        for (line, code) in outside_tests(&text) {
            if branches_on_the_amp(code) {
                found.push(format!("{}:{line}: {}", shown(&path), code.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "the amp's DSP branches on which amp it is:\n{}\n\nAmps differ only by constants in \
         `amp/model.rs` (Guitar Amp in plugins/README.md): give `AmpModel` a field for the difference, set \
         it for each amp there, and read it from `amp.model()`",
        found.join("\n")
    );
}

#[test]
fn test_what_counts_as_a_branch_on_the_amp() {
    for branch in [
        "match self.amp {",
        "match settings.wanted_amp {",
        "Amp::Klar => 1.0,",
        "Amp::Brol | Amp::Glod => 2.0,",
        "if amp == Amp::Brol {",
        "if amp != Amp::Torden {",
        "if settings.amp == amp::model::Amp::Klar {}",
        "if let Amp::Klar = amp {",
        "let tight = matches!(amp, Amp::Torden);",
    ] {
        assert!(branches_on_the_amp(branch), "not seen: {branch}");
    }
    for fine in [
        "for amp in Amp::ALL {",
        "amp: Amp::Brol,",
        "let amp = Amp::default();",
        "let model = settings.amp.model();",
        "match self.mic_type {",
        "let x = clamp(y);",
    ] {
        assert!(!branches_on_the_amp(fine), "taken for a branch: {fine}");
    }
}

#[test]
fn test_only_tests_are_left_out() {
    let text = "#[cfg(test)]
mod test_util;
                fn a() { match self.amp {} }
                #[cfg(test)]
#[allow(unused)]
fn helper() {
    match amp {}
}
                fn b() {}
                #[cfg(test)]
mod tests {
    fn c() { if amp == Amp::Klar {} }
}
                fn d() {} // match amp
";
    let kept: Vec<usize> = outside_tests(text).into_iter().map(|(line, _)| line).collect();
    assert_eq!(kept, [3, 9, 14]);
    let branches: Vec<_> =
        outside_tests(text).into_iter().filter(|(_, code)| branches_on_the_amp(code)).collect();
    assert_eq!(branches.len(), 1, "{branches:?}");
}

#[test]
fn test_a_listed_name_is_found() {
    // Made-up names, as no real one may be written here: one word and two, also written as
    // a type's name and across a line
    let names = [fnv1a("zorblax"), fnv1a("quint vexel")];
    let text = "struct ZorblaxStage;
// a quint
// vexel in the cabinet
fn quintet() {}";
    let found: Vec<(usize, String)> = named(text, &names);
    assert_eq!(found, [(1, "zorblax".to_string()), (3, "quint vexel".to_string())]);
}

/// Prints the hash of `NAME`, for adding a name to [`NAMES`]
#[test]
#[ignore]
fn hash_of_name() {
    let name = std::env::var("NAME").expect("NAME=\"the name\"");
    println!(
        "    0x{:016x},",
        fnv1a(&words(&name).into_iter().map(|w| w.1).collect::<Vec<_>>().join(" "))
    );
}

/// The words of `text`, and pairs of them in a row, that are among `names`, each with its line
fn named(text: &str, names: &[u64]) -> Vec<(usize, String)> {
    let words = words(text);
    let pairs = words.windows(2).map(|pair| (pair[1].0, format!("{} {}", pair[0].1, pair[1].1)));
    words.iter().cloned().chain(pairs).filter(|(_, name)| names.contains(&fnv1a(name))).collect()
}

/// The lines of `text` that are not tests, with their numbers, without their comments. Tests
/// may go through the amps one by one: the item after each `#[cfg(test)]` is left out, up to
/// its `;` or the brace that closes it
fn outside_tests(text: &str) -> Vec<(usize, &str)> {
    let mut kept = Vec::new();
    let mut in_test = false;
    let mut depth = 0i32;
    let mut opened = false;
    for (i, line) in text.lines().enumerate() {
        let code = line.split("//").next().unwrap();
        if code.trim_start().starts_with("#[cfg(test)]") {
            (in_test, depth, opened) = (true, 0, false);
            continue;
        }
        if !in_test {
            kept.push((i + 1, code));
            continue;
        }
        for c in code.chars() {
            match c {
                '{' => (depth, opened) = (depth + 1, true),
                '}' => depth -= 1,
                _ => {}
            }
        }
        let attribute = !opened && code.trim_start().starts_with("#[");
        if !attribute && ((opened && depth <= 0) || (!opened && code.trim_end().ends_with(';'))) {
            in_test = false;
        }
    }
    kept
}

/// A `match` on something named amp, a match arm or comparison with one amp
fn branches_on_the_amp(line: &str) -> bool {
    if let Some((_, rest)) = line.split_once("match ") {
        let scrutinee = rest.split('{').next().unwrap();
        if scrutinee
            .split(|c: char| !c.is_alphanumeric())
            .any(|part| part.eq_ignore_ascii_case("amp"))
        {
            return true;
        }
    }
    line.match_indices("Amp::").any(|(at, _)| {
        let after = &line[at + "Amp::".len()..];
        let variant: String = after.chars().take_while(|c| c.is_alphanumeric()).collect();
        let is_variant = variant.chars().next().is_some_and(char::is_uppercase) && variant != "ALL";
        let rest = after[variant.len()..].trim_start();
        // What comes before the path the variant is named by, as in `model::Amp::Klar`
        let before = line[..at]
            .trim_end_matches(|c: char| c.is_alphanumeric() || c == '_' || c == ':')
            .trim_end();
        is_variant
            && (rest.starts_with("=>")
                || rest.starts_with('|')
                || before.ends_with("==")
                || before.ends_with("!=")
                || before.ends_with("let")
                || line.contains("matches!("))
    })
}

/// The words of `text`, lowercase, with the line each is on: letters and digits, also split
/// where a lowercase letter meets a capital, as in a type's name
fn words(text: &str) -> Vec<(usize, String)> {
    let mut words = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let mut word = String::new();
        let mut last_lower = false;
        for c in line.chars().chain([' ']) {
            if c.is_alphanumeric() && !(last_lower && c.is_uppercase()) {
                word.extend(c.to_lowercase());
            } else if !word.is_empty() {
                words.push((i + 1, std::mem::take(&mut word)));
                if c.is_alphanumeric() {
                    word.extend(c.to_lowercase());
                }
            }
            last_lower = c.is_lowercase();
        }
    }
    words
}

fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x100_0000_01b3)
    })
}

fn amp_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every file under `dir` that `keep` keeps
fn files(dir: &Path, keep: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(files(&path, keep));
        } else if keep(&path) {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// `path` from the amp's folder, with forward slashes
fn shown(path: &Path) -> String {
    path.strip_prefix(amp_dir()).unwrap().to_string_lossy().replace('\\', "/")
}
