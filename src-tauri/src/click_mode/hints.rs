//! Hint label generation for Click Mode
//!
//! Generates short, easy-to-type labels for clickable elements.
//! Uses home row keys first for fastest access.

#![allow(dead_code)]

/// Default hint characters - home row first, then other rows
pub const DEFAULT_HINT_CHARS: &str = "asdfghjklqwertyuiopzxcvbnm";

/// Generate hint labels for a given number of elements
///
/// All hints have the same length to avoid prefix conflicts.
/// Uses shortest possible length that can generate enough unique hints.
///
/// # Arguments
/// * `count` - Number of hints needed
/// * `chars` - Characters to use for hints (default: home row first)
///
/// # Returns
/// Vector of hint strings, length equal to `count`
pub fn generate_hints(count: usize, chars: &str) -> Vec<String> {
    if count == 0 {
        return Vec::new();
    }

    let chars: Vec<char> = chars.chars().collect();
    if chars.is_empty() {
        return (0..count).map(|i| i.to_string()).collect();
    }

    let base = chars.len();

    // Calculate minimum hint length needed
    // With N chars, we can have N^1 single-char, N^2 two-char, etc.
    let hint_length = if count <= base {
        1
    } else if count <= base * base {
        2
    } else {
        3 // Should be enough for most cases (26^3 = 17576)
    };

    let mut hints = Vec::with_capacity(count);

    match hint_length {
        1 => {
            for c in chars.iter().take(count) {
                hints.push(c.to_string().to_uppercase());
            }
        }
        2 => {
            'outer: for c1 in &chars {
                for c2 in &chars {
                    if hints.len() >= count {
                        break 'outer;
                    }
                    hints.push(format!("{}{}", c1, c2).to_uppercase());
                }
            }
        }
        _ => {
            'outer: for c1 in &chars {
                for c2 in &chars {
                    for c3 in &chars {
                        if hints.len() >= count {
                            break 'outer;
                        }
                        hints.push(format!("{}{}{}", c1, c2, c3).to_uppercase());
                    }
                }
            }
        }
    }

    hints
}

/// Check if a hint matches the current input buffer
///
/// # Arguments
/// * `hint` - The full hint label
/// * `input` - The current input buffer
///
/// # Returns
/// - `Some(true)` if exact match (should activate)
/// - `Some(false)` if partial match (keep waiting)
/// - `None` if no match (filter out)
pub fn match_hint(hint: &str, input: &str) -> Option<bool> {
    let hint_upper = hint.to_uppercase();
    let input_upper = input.to_uppercase();

    if hint_upper == input_upper {
        Some(true) // Exact match
    } else if hint_upper.starts_with(&input_upper) {
        Some(false) // Partial match
    } else {
        None // No match
    }
}

/// Filter elements by hint prefix
///
/// # Arguments
/// * `hints` - Slice of hint strings
/// * `input` - Current input buffer
///
/// # Returns
/// Indices of hints that match or partially match the input
pub fn filter_by_prefix(hints: &[String], input: &str) -> Vec<usize> {
    hints
        .iter()
        .enumerate()
        .filter_map(|(i, hint)| match_hint(hint, input).map(|_| i))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_single_char_hints() {
        let hints = generate_hints(5, "asdfg");
        assert_eq!(hints, vec!["A", "S", "D", "F", "G"]);
    }

    #[test]
    fn test_generate_two_char_hints_when_needed() {
        // With 2 chars (a, b), we can only have 2 single-char hints
        // So 3+ elements need 2-char hints for ALL elements
        let hints = generate_hints(3, "ab");
        assert_eq!(hints, vec!["AA", "AB", "BA"]);
    }

    #[test]
    fn test_generate_two_char_hints_exact_boundary() {
        // Exactly at boundary - 2 elements with 2 chars = single char hints
        let hints = generate_hints(2, "ab");
        assert_eq!(hints, vec!["A", "B"]);
    }

    #[test]
    fn test_no_prefix_conflicts() {
        // With 26 chars and 27 elements, all should be 2-char
        let hints = generate_hints(27, DEFAULT_HINT_CHARS);
        assert_eq!(hints.len(), 27);
        // All hints should be 2 chars
        assert!(hints.iter().all(|h| h.len() == 2));
        // First hint should be "AA" (first char repeated)
        assert_eq!(hints[0], "AA");
        // No hint should be a prefix of another
        for (i, h1) in hints.iter().enumerate() {
            for (j, h2) in hints.iter().enumerate() {
                if i != j {
                    assert!(!h2.starts_with(h1), "{} is prefix of {}", h1, h2);
                }
            }
        }
    }

    #[test]
    fn test_generate_empty() {
        let hints = generate_hints(0, "abc");
        assert!(hints.is_empty());
    }

    #[test]
    fn test_match_hint_exact() {
        assert_eq!(match_hint("AB", "ab"), Some(true));
        assert_eq!(match_hint("AB", "AB"), Some(true));
    }

    #[test]
    fn test_match_hint_partial() {
        assert_eq!(match_hint("AB", "a"), Some(false));
        assert_eq!(match_hint("ABC", "ab"), Some(false));
    }

    #[test]
    fn test_match_hint_none() {
        assert_eq!(match_hint("AB", "c"), None);
        assert_eq!(match_hint("AB", "ba"), None);
    }

    #[test]
    fn test_filter_by_prefix() {
        // All hints same length now, so filtering works differently
        let hints = vec![
            "AA".to_string(),
            "AB".to_string(),
            "AC".to_string(),
            "BA".to_string(),
        ];
        let filtered = filter_by_prefix(&hints, "a");
        assert_eq!(filtered, vec![0, 1, 2]); // AA, AB, AC match "a"
    }
}
