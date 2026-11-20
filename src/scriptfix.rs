//! Corrections to the game's own scripts (usecode), made when they are
//! loaded. Each one looks for the exact bytes it corrects and leaves the
//! scripts as they are when it does not find them (another language,
//! another version).
//!
//! Stellos after the third test (German): once the foci are made, his
//! menu offers "Ich habe die Dritte Prüfung bestanden." with a capital D,
//! the answer of the time before the foci ("Herzlichen Glückwunsch. Jetzt
//! müßt Ihr nur noch Eure Zaubersymbole anfertigen.", and again how foci
//! are made). The answer meant for it is waiting for "Ich habe die dritte
//! Prüfung bestanden." with a small d ("Das ist wunderbar. Aber ...
//! Etwas Schreckliches ist geschehen.": Xavier's focus has gone missing),
//! as in the English and French scripts, where the two lines differ.
//! Here the line offered gets its small d.

/// Stellos's class (NPC 23).
const STELLOS: usize = 0x417;

/// Code offsets inside a class start after this many bytes.
const BASE: usize = 0x0C;

/// Applies the corrections to the classes as loaded (index = class).
pub fn apply(classes: &mut [Option<Vec<u8>>]) {
    if let Some(Some(code)) = classes.get_mut(STELLOS) {
        third_test_line(code);
    }
}

/// `passedThirdTest and hasOwnFoci`: the line Stellos's menu then offers.
fn third_test_line(code: &mut [u8]) -> bool {
    // push global passedThirdTest (bit 0x82), push global hasOwnFoci (0x80),
    // and, jump if not (2 bytes), push string
    const BEFORE: [u8; 10] = [0x4E, 0x82, 0x00, 0x01, 0x4E, 0x80, 0x00, 0x01, 0x32, 0x51];
    const WRONG: &[u8] = b" Ich habe die Dritte Pr\x81fung bestanden.";
    const MEANT: &[u8] = b" Ich habe die dritte Pr\x81fung bestanden.";
    let Some(at) = code.windows(BEFORE.len()).position(|w| w == BEFORE) else { return false };
    // the string: opcode 0x0D, its length (2 bytes), the text
    let s = at + BEFORE.len() + 2;
    let text = s + 3;
    let fits = code.get(s) == Some(&0x0D)
        && code.get(s + 1..s + 3) == Some(&(WRONG.len() as u16).to_le_bytes()[..])
        && code.get(text..text + WRONG.len()) == Some(WRONG);
    // only where the answer for the line with the small d is there
    if !fits || !code[BASE..].windows(MEANT.len()).any(|w| w == MEANT) {
        return false;
    }
    code[text..text + MEANT.len()].copy_from_slice(MEANT);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(line: &[u8]) -> Vec<u8> {
        let mut c = vec![0u8; BASE];
        c.extend_from_slice(&[0x4E, 0x82, 0x00, 0x01, 0x4E, 0x80, 0x00, 0x01, 0x32, 0x51, 0x10, 0x00, 0x0D]);
        c.extend_from_slice(&(line.len() as u16).to_le_bytes());
        c.extend_from_slice(line);
        c.push(0);
        // the answer further on
        c.push(0x0D);
        c.extend_from_slice(b" Ich habe die dritte Pr\x81fung bestanden.");
        c
    }

    #[test]
    fn the_line_gets_its_small_d() {
        let mut c = class(b" Ich habe die Dritte Pr\x81fung bestanden.");
        assert!(third_test_line(&mut c));
        assert_eq!(c.windows(6).filter(|w| w == b"dritte").count(), 2);
        assert!(!c.windows(6).any(|w| w == b"Dritte"));
    }

    #[test]
    fn other_scripts_stay_as_they_are() {
        let mut c = class(b" I have passed the third test. ");
        let before = c.clone();
        assert!(!third_test_line(&mut c));
        assert_eq!(c, before);
    }

    /// The German scripts of the game, if they lie beside this checkout.
    #[test]
    fn german_scripts() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("GERMAN").join("USECODE").join("GUSECODE.FLX");
        let Ok(flex) = crate::flex::Flex::open(&path) else { return };
        let Some(mut code) = flex.get(STELLOS + 2).map(|d| d.to_vec()) else { return };
        assert!(third_test_line(&mut code));
        // the English ones have nothing to correct
        let path = path.parent().unwrap().parent().unwrap().parent().unwrap().join("ENGLISH").join("USECODE").join("EUSECODE.FLX");
        if let Ok(flex) = crate::flex::Flex::open(&path) {
            let mut code = flex.get(STELLOS + 2).unwrap().to_vec();
            assert!(!third_test_line(&mut code));
        }
    }
}
