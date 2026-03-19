use std::collections::HashMap;

/// Replace tool names inside backtick spans using the provided mappings.
///
/// Only whole words inside `` ` `` spans are replaced. Prose text outside
/// backticks is never modified.
///
/// ```
/// # use std::collections::HashMap;
/// # use commands::transform::remap_tools;
/// let mut mappings = HashMap::new();
/// mappings.insert("Read".to_string(), "read_file".to_string());
/// let result = remap_tools("Use `Read` to read. Read the docs.", &mappings);
/// assert_eq!(result, "Use `read_file` to read. Read the docs.");
/// ```
pub fn remap_tools(
    content: &str,
    mappings: &HashMap<String, String, impl std::hash::BuildHasher>,
) -> String {
    if mappings.is_empty() {
        return content.to_string();
    }

    let mut result = String::with_capacity(content.len());
    let mut remaining = content;

    // Walk through the content, finding backtick pairs.
    //
    // Input:  "Use `Read` to read. `Bash` runs commands. Read the docs."
    //         ─────┬────────────── ──┬───────────────── ─────┬──────────
    //           prose (untouched)   backtick span         prose (untouched)
    //
    // For each backtick span, replace whole words:
    //   `Read`  → mappings["Read"] = "read_file"  → `read_file`
    //   `Bash`  → mappings["Bash"] = "shell"       → `shell`
    //   "Read the docs." stays unchanged (no backticks)
    loop {
        let Some(open_pos) = remaining.find('`') else {
            result.push_str(remaining);
            break;
        };

        // Everything before the opening backtick is prose — copy verbatim
        result.push_str(&remaining[..open_pos]);
        let after_open = &remaining[open_pos + 1..];

        let Some(close_pos) = after_open.find('`') else {
            // Unclosed backtick at end of content — copy as-is
            result.push_str(&remaining[open_pos..]);
            break;
        };

        let span = &after_open[..close_pos];

        // Replace whole words within the span:
        //   "Read"       → "read_file"     (exact match)
        //   "Read/Write" → "read_file/write_file" (two words separated by /)
        //   "Reading"    → "Reading"        (no match — "Reading" != "Read")
        let remapped = remap_span_words(span, mappings);
        result.push('`');
        result.push_str(&remapped);
        result.push('`');

        remaining = &after_open[close_pos + 1..];
    }

    result
}

/// Replace whole words in a backtick span using the mappings.
///
/// Words are delimited by non-alphanumeric, non-underscore characters.
/// Only exact whole-word matches are replaced.
///
/// Example: span `Read/Write` with mappings {`Read`→`read_file`, `Write`→`write_file`}
///   - index 0..4 = `Read` → `read_file`
///   - index 4 = `/` → `/`
///   - index 5..10 = `Write` → `write_file`
///   - result: `read_file/write_file`
fn remap_span_words(
    span: &str,
    mappings: &HashMap<String, String, impl std::hash::BuildHasher>,
) -> String {
    let mut result = String::with_capacity(span.len());
    let mut word_start = 0;
    let characters: Vec<char> = span.chars().collect();

    for index in 0..=characters.len() {
        let is_word_char = index < characters.len()
            && (characters[index].is_alphanumeric() || characters[index] == '_');

        // When we hit a non-word character (or end of span), emit the accumulated word
        if !is_word_char && index > word_start {
            let word: String = characters[word_start..index].iter().collect();
            if let Some(mapped) = mappings.get(&word) {
                result.push_str(mapped);
            } else {
                result.push_str(&word);
            }
        }

        // Emit non-word characters (delimiters like '/', ' ', etc.) verbatim
        if !is_word_char && index < characters.len() {
            result.push(characters[index]);
        }

        if !is_word_char {
            word_start = index + 1;
        }
    }

    result
}
