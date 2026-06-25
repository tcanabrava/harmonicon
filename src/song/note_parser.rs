#[derive(Debug, PartialEq, Copy, Clone)]
enum State {
    Start,          // Beginning of the string or right after a space
    Minus,          // Saw a '-' sign, expecting a digit
    Number,         // Parsing a number (1-9 or 10)
    Rest,           // Saw 'r'
    Duration,       // Saw 'w', 'h', 'q', or 'e'
    Space,          // Saw a valid space, expecting a new note
    Bend,            // Saw a Bend ', expecting a new note or a bend.
    Failed,         // Invalid sequence encountered
}

struct MatchResult {
    matched: bool,
    hit_end: bool,
}

fn analyze_notes(bytes: &[u8]) -> MatchResult {
    if bytes.is_empty() {
        // Regex uses ^...$, and while the internal groups are optional,
        // it requires at least one note base to start. Empty string fails.
        return MatchResult { matched: false, hit_end: true };
    }

    let mut state = State::Start;
    let mut current_num: u32 = 0;
    let mut last_bend = 0;

    for &b in bytes {
        state = match state {
            State::Start => match b {
                b'-' => State::Minus,
                b'r' => State::Rest,
                b'1'..=b'9' => {
                    current_num = (b - b'0') as u32;
                    State::Number
                }
                _ => State::Failed,
            },
            State::Minus => match b {
                b'1'..=b'9' => {
                    current_num = (b - b'0') as u32;
                    State::Number
                }
                _ => State::Failed,
            },
            State::Number => match b {
                // If it's another digit, check if it forms '10'
                b'0'..=b'9' => {
                    current_num = current_num * 10 + ((b - b'0') as u32);
                    if current_num == 10 {
                        State::Number
                    } else {
                        State::Failed // Out of bounds (> 10)
                    }
                }
                b'\''=> State::Bend,
                b'w' | b'h' | b'q' | b'e' => State::Duration,
                b' ' => State::Space, // whitespace group \s
                _ => State::Failed,
            },
            State::Rest => match b {
                b'w' | b'h' | b'q' | b'e' => State::Duration,
                b' ' => State::Space,
                _ => State::Failed,
            },
            State::Duration => match b {
                b' ' => State::Space,
                b'\'' => State::Bend,
                _ => State::Failed,
            },
            State::Bend => match b {
                b' ' => State::Space,
                b'\'' => {
                    last_bend += 1;
                    if last_bend == 2 {
                        State::Failed
                    } else {
                        State::Bend
                    }
                }
                _ => State::Failed,
            },
            State::Space => match b {
                // Absorb consecutive spaces, or transition to a new note token
                b' ' => State::Space,
                b'-' => State::Minus,
                b'r' => State::Rest,
                b'1'..=b'9' => {
                    current_num = (b - b'0') as u32;
                    State::Number
                }
                _ => State::Failed,
            },
            State::Failed => State::Failed,
        };

        if state == State::Failed {
            return MatchResult { matched: false, hit_end: false };
        }
    }

    // Evaluate final state when stream terminates
    match state {
        // Safe terminal positions that represent a complete, valid string
        State::Number | State::Rest | State::Duration | State::Bend => MatchResult {
            matched: true,
            hit_end: true // True because appending 'w', 'h', 'q', 'e', or ' [note]' is valid!
        },
        // Mid-token positions
        State::Start | State::Minus | State::Space => MatchResult {
            matched: false,
            hit_end: true
        },
        State::Failed => MatchResult {
            matched: false,
            hit_end: false
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper macro to clean up test assertions
    macro_rules! assert_analysis {
        ($input:expr, $matched:expr, $hit_end:expr) => {
            let res = analyze_notes($input.as_bytes());
            assert_eq!(
                res.matched, $matched,
                "Expected matched={} for {:?}", $matched, $input
            );
            assert_eq!(
                res.hit_end, $hit_end,
                "Expected hit_end={} for {:?}", $hit_end, $input
            );
        };
    }

    #[test]
    fn test_empty_input() {
        // Empty string can't match, but adding text can make it match
        assert_analysis!("", false, true);
    }

    #[test]
    fn test_single_valid_notes() {
        // Single base values
        assert_analysis!("5", true, true);
        assert_analysis!("10", true, true);
        assert_analysis!("-3", true, true);
        assert_analysis!("-10", true, true);
        assert_analysis!("r", true, true);

        // Single values with duration suffixes
        assert_analysis!("5w", true, true);
        assert_analysis!("10h", true, true);
        assert_analysis!("-3q", true, true);
        assert_analysis!("re", true, true);
    }

    #[test]
    fn test_multiple_valid_notes() {
        // Space separated sequences
        assert_analysis!("5 -10w r", true, true);
        assert_analysis!("r 10e -1 2h", true, true);
        // Consecutive spacing tokens (\s+)
        assert_analysis!("5   -3''   r", true, true);
    }

    #[test]
    fn test_partial_matches_hit_end() {
        // Cut off mid-token on minus sign
        assert_analysis!("-", false, true);
        assert_analysis!("5w -", false, true);

        // Cut off while parsing a number that could become 10
        assert_analysis!("1", true, true); // Valid '1', but '0' could follow

        // Cut off on trailing spaces (expecting another note)
        assert_analysis!("5' ", false, true);
        assert_analysis!("r ", false, true);
    }

    #[test]
    fn test_hard_failures_not_hit_end() {
        // Numbers out of bounds (> 10)
        assert_analysis!("11", false, false);
        assert_analysis!("-12", false, false);
        assert_analysis!("5 25", false, false);

        // Zero is invalid based on [1-9]|10
        assert_analysis!("0", false, false);
        assert_analysis!("-0", false, false);

        // Invalid duration suffix characters
        assert_analysis!("5x", false, false);
        assert_analysis!("rx", false, false);

        // Text after a complete suffix without spacing
        assert_analysis!("5w1", false, false);
        assert_analysis!("r-", false, false);

        // Completely invalid characters
        assert_analysis!("abc", false, false);
        assert_analysis!("5 +3", false, false);
    }
}
