//! Streaming shell-semantic protocol handling.
//!
//! This is deliberately narrower than a VT parser. `libghostty-vt` remains
//! responsible for terminal state; this reducer only observes OSC 133 markers
//! so command lifecycle can work until Ghostty exposes OSC dispatch callbacks.

/// A shell-semantic event decoded from OSC 133.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticEvent {
    /// A new shell prompt has started.
    PromptStarted,
    /// The shell's editable command input has started.
    InputStarted,
    /// The command's output region has started.
    OutputStarted,
    /// The shell has completed a command.
    CommandCompleted { exit_status: Option<i32> },
}

#[derive(Clone, Debug, Default)]
enum ParserState {
    #[default]
    Ground,
    Escape,
    Osc(Vec<u8>),
    OscEscape(Vec<u8>),
}

/// A chunk-boundary-safe OSC 133 observer.
#[derive(Clone, Debug, Default)]
pub struct SemanticDecoder {
    state: ParserState,
}

impl SemanticDecoder {
    /// Feeds PTY output into the decoder and returns semantic events in order.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<SemanticEvent> {
        bytes
            .iter()
            .copied()
            .flat_map(|byte| self.feed_byte(byte))
            .collect()
    }

    fn feed_byte(&mut self, byte: u8) -> Option<SemanticEvent> {
        let state = std::mem::take(&mut self.state);
        match state {
            ParserState::Ground => match byte {
                0x1b => {
                    self.state = ParserState::Escape;
                    None
                }
                0x9d => {
                    self.state = ParserState::Osc(Vec::new());
                    None
                }
                _ => None,
            },
            ParserState::Escape => match byte {
                b']' => {
                    self.state = ParserState::Osc(Vec::new());
                    None
                }
                _ => {
                    self.state = ParserState::Ground;
                    None
                }
            },
            ParserState::Osc(mut payload) => match byte {
                0x07 | 0x9c => finish_osc(&mut self.state, payload),
                0x1b => {
                    self.state = ParserState::OscEscape(payload);
                    None
                }
                _ => {
                    payload.push(byte);
                    self.state = ParserState::Osc(payload);
                    None
                }
            },
            ParserState::OscEscape(payload) => match byte {
                b'\\' => finish_osc(&mut self.state, payload),
                _ => {
                    self.state = ParserState::Ground;
                    None
                }
            },
        }
    }
}

fn finish_osc(state: &mut ParserState, payload: Vec<u8>) -> Option<SemanticEvent> {
    *state = ParserState::Ground;
    parse_osc133(&payload)
}

fn parse_osc133(payload: &[u8]) -> Option<SemanticEvent> {
    let body = payload.strip_prefix(b"133;")?;
    let (marker, parameters) = body
        .split_first()
        .map(|(marker, rest)| (*marker, rest.strip_prefix(b";").unwrap_or(rest)))?;

    match marker {
        b'A' | b'P' => Some(SemanticEvent::PromptStarted),
        b'B' => Some(SemanticEvent::InputStarted),
        b'C' => Some(SemanticEvent::OutputStarted),
        b'D' => Some(SemanticEvent::CommandCompleted {
            exit_status: parameters
                .split(|byte| *byte == b';')
                .next()
                .and_then(|status| std::str::from_utf8(status).ok())
                .and_then(|status| status.parse().ok()),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_markers_split_across_input_chunks() {
        let mut decoder = SemanticDecoder::default();

        assert!(decoder.feed(b"\x1b]13").is_empty());
        assert_eq!(
            decoder.feed(b"3;B\x1b\\").as_slice(),
            [SemanticEvent::InputStarted]
        );
        assert_eq!(
            decoder.feed(b"\x1b]133;D;7;aid=1\x07").as_slice(),
            [SemanticEvent::CommandCompleted {
                exit_status: Some(7)
            }]
        );
    }

    #[test]
    fn accepts_bel_and_eight_bit_osc_terminators() {
        let mut decoder = SemanticDecoder::default();

        assert_eq!(
            decoder.feed(b"\x1b]133;A\x07").as_slice(),
            [SemanticEvent::PromptStarted]
        );
        assert_eq!(
            decoder.feed(b"\x9d133;C\x9c").as_slice(),
            [SemanticEvent::OutputStarted]
        );
    }

    #[test]
    fn ignores_other_osc_commands_and_malformed_statuses() {
        let mut decoder = SemanticDecoder::default();

        assert!(decoder.feed(b"\x1b]7;file:///tmp\x1b\\").is_empty());
        assert_eq!(
            decoder.feed(b"\x1b]133;D;not-a-number\x1b\\").as_slice(),
            [SemanticEvent::CommandCompleted { exit_status: None }]
        );
    }
}
