//! Raw MIDI bytes → engine events.

use crate::engine::Event;

const CC_SUSTAIN: u8 = 64;

pub fn parse(bytes: &[u8]) -> Option<Event> {
    let (&status, data) = bytes.split_first()?;
    match (status & 0xF0, data) {
        (0x90, [key, vel, ..]) => Some(Event::NoteOn { key: *key & 0x7F, velocity: *vel & 0x7F }),
        (0x80, [key, ..]) => Some(Event::NoteOff { key: *key & 0x7F }),
        (0xB0, [CC_SUSTAIN, value, ..]) => Some(Event::Sustain((*value & 0x7F) as f32 / 127.0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_notes_and_sustain() {
        assert_eq!(parse(&[0x91, 60, 100]), Some(Event::NoteOn { key: 60, velocity: 100 }));
        assert_eq!(parse(&[0x80, 60, 0]), Some(Event::NoteOff { key: 60 }));
        assert_eq!(parse(&[0xB0, 64, 127]), Some(Event::Sustain(1.0)));
        assert_eq!(parse(&[0xB0, 1, 127]), None);
        assert_eq!(parse(&[]), None);
    }
}
