use bevy::{core_pipeline::deferred::DEFERRED_LIGHTING_PASS_ID_FORMAT, material::bind_group_layout_entries};

#[derive(Debug, PartialEq, Copy, Clone)]
enum State {
    Start,          // Beginning of the string or right after a space
    Minus,          // Saw a '-' sign, expecting a digit
    Number,         // Parsing a number (1-9 or 10)
    Rest,           // Saw 'r'
    OpenParenthesis, // Saw '('
    Comma,          // Saw ','
    CloseParenthesis, // Saw ')'
    Duration,       // Saw 'w', 'h', 'q', 'e', or 's'
    Space,          // Saw a valid space, expecting a new note
    Failed,         // Invalid sequence encountered
}

pub struct MatchResult {
    pub matched: bool,
    pub is_valid: bool,
}

pub fn analyze_notes(bytes: &[u8]) -> MatchResult {
    if bytes.is_empty() {
        // Regex uses ^...$, and while the internal groups are optional,
        // it requires at least one note base to start. Empty string fails.
        println!("Empty notes to analyze");
        return MatchResult { matched: false, is_valid: false };
    }
    let input = str::from_utf8(bytes).unwrap().to_string();
    println!("Analyzing {}", input);

    let mut state = State::Start;
    let mut current_num: u32 = 0;
    let mut has_minus = false;
    let mut in_parentheses = false;

    for &b in bytes {
        state = match state {
            State::Start => match b {
                b'-' => {
                    has_minus = true;
                    State::Minus
                },
                b'r' => State::Rest,
                // Holes are 1-10, so a token never starts with 0.
                b'1'..=b'9' => {
                    current_num = (b - b'0') as u32;
                    State::Number
                }
                b'(' => {
                    in_parentheses = true;
                    State::OpenParenthesis
                },
                _ => State::Failed,
            },
            State::Minus => match b {
                b'1'..=b'9' => {
                    has_minus = false;
                    current_num = (b - b'0') as u32;
                    State::Number
                }
                b'(' => {
                    in_parentheses = true;
                    State::OpenParenthesis
                },
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
                b'w' | b'h' | b'q' | b'e' | b's' => State::Duration,
                b',' => {
                    if in_parentheses {
                        State::Comma
                    } else {
                        State::Failed
                    }
                }
                b')' => {
                    in_parentheses = false;
                    has_minus = false;
                    State::CloseParenthesis
                }
                b' ' => State::Space, // whitespace group \s
                _ => State::Failed,
            },
            State::OpenParenthesis => match b {
                b'1'..=b'9' => {
                    current_num = (b - b'0') as u32;
                    State::Number
                }
                _ => State::Failed,
            },
            State::Comma => match b {
                b'1'..=b'9' => {
                    current_num = (b - b'0') as u32;
                    State::Number
                }
                _ => State::Failed,
            },
            State::CloseParenthesis => match b {
                b' ' => State::Space,
                b'w' | b'h' | b'q' | b'e' | b's' => State::Duration,
                _ => State::Failed,
            },
            State::Rest => match b {
                b'w' | b'h' | b'q' | b'e' | b's' => State::Duration,
                b' ' => State::Space,
                _ => State::Failed,
            },
            State::Duration => match b {
                b' ' => State::Space,
                _ => State::Failed,
            },
            State::Space => match b {
                // Absorb consecutive spaces, or transition to a new note token
                b' ' => State::Space,
                b'-' => {
                    has_minus = true;
                    State::Minus
                },
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
            println!("Failed to parse note at byte {:?}", b);
            return MatchResult { matched: false, is_valid: false };
        }
    }

    // Evaluate final state when stream terminates
    let res = match state {
        // Safe terminal positions that represent a complete, valid string
        State::Failed => MatchResult {
            matched: false,
            is_valid: false,
        },
        _ => MatchResult {
            matched: true,
            is_valid: !has_minus,
        },
    };

    println!("Analyzed notes: matched={} is_valid={}", res.matched, res.is_valid);
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper macro to clean up test assertions
    macro_rules! assert_analysis {
        ($input:expr, $matched:expr, $is_valid:expr) => {
            let res = analyze_notes($input.as_bytes());
            assert_eq!(
                res.matched, $matched,
                "Expected matched={} for {:?}", $matched, $input
            );
            assert_eq!(
                res.is_valid, $is_valid,
                "Expected is_valid={} for {:?}", $is_valid, $input
            );
        };
    }

    #[test]
    fn test_empty_input() {
        // Empty string can't match, but adding text can make it match
        assert_analysis!("", false, false);
    }

    #[test]
    fn test_single_valid_notes() {
        // Single base values
        assert_analysis!("5", true, true);
        assert_analysis!("10", true, true);
        assert_analysis!("-3", true, true);
        assert_analysis!("-10", true, true);
        assert_analysis!("r", true, true);
        assert_analysis!("-", true, false);

        // Single values with duration suffixes (w/h/q/e/s)
        assert_analysis!("5w", true, true);
        assert_analysis!("10h", true, true);
        assert_analysis!("-3q", true, true);
        assert_analysis!("4s", true, true);
        assert_analysis!("re", true, true);
        assert_analysis!("(1)", true, true);
        assert_analysis!("(1,3)", true, true);
        assert_analysis!("(1,2,3)", true, true);
        assert_analysis!("-(1)", true, true);
        assert_analysis!("-(1,3)", true, true);
        assert_analysis!("-(1,2,3)", true, true);
    }

    #[test]
    fn test_multiple_valid_notes() {
        // Space separated sequences
        assert_analysis!("5 -10w r", true, true);
        assert_analysis!("r 10e -1 2h", true, true);
        // Consecutive spacing tokens (\s+)
        assert_analysis!("5   -3   r", true, true);
        assert_analysis!("4 -(1)", true, true);
        assert_analysis!("1 -(1,3) 4 ", true, true);
        assert_analysis!("1 -(1,2,3)h 5", true, true);
    }

    #[test]
    fn test_partial_matches_hit_end() {
        // Cut off mid-token on minus sign
        assert_analysis!("-", true, false);
        assert_analysis!("5w -", true, false);

        // Cut off while parsing a number that could become 10
        assert_analysis!("1", true, true); // Valid '1', but '0' could follow

        // Cut off on trailing spaces (expecting another note)
        assert_analysis!("5 ", true, true);
        assert_analysis!("r ", true, true);
    }

    #[test]
    fn test_hard_failures_not_hit_end() {
        // Numbers out of bounds (> 10)
        assert_analysis!("11", false, false);
        assert_analysis!("-12", false, false);
        assert_analysis!("5 25", false, false);

        // Hole 0 doesn't exist (valid range is 1-10).
        assert_analysis!("0", false, false);
        assert_analysis!("-0", false, false);

        // Bends are a note modifier (applied in the editor), not part of the
        // tab text, so an apostrophe is not a valid character here.
        assert_analysis!("5'", false, false);

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
