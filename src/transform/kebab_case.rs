/// Convert a `PascalCase` name to kebab-case.
///
/// Inserts `-` at two boundary types:
/// - lowercase/digit followed by uppercase (`gameM` → `game-m`)
/// - uppercase followed by uppercase+lowercase (`XMLP` → `xml-p`)
///
/// A single lowercase letter between two uppercase letters is treated as part
/// of an abbreviation, not a word boundary (`DnD` stays `dnd`, not `dn-d`).
///
/// Spaces and underscores become `-`. Consecutive hyphens collapse.
///
/// ```
/// # use commands::transform::to_kebab_case;
/// assert_eq!(to_kebab_case("SecurityArchitect"), "security-architect");
/// assert_eq!(to_kebab_case("XMLParser"), "xml-parser");
/// assert_eq!(to_kebab_case("DnDBeyondHomebrew"), "dnd-beyond-homebrew");
/// ```
pub fn to_kebab_case(name: &str) -> String {
    let characters: Vec<char> = name.chars().collect();
    let mut raw = String::with_capacity(name.len() + 4);

    for (index, &character) in characters.iter().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 {
                let previous = characters[index - 1];
                let previous_was_lower_or_digit =
                    previous.is_ascii_lowercase() || previous.is_ascii_digit();
                let previous_was_upper = previous.is_ascii_uppercase();
                let next_is_lower = characters
                    .get(index + 1)
                    .is_some_and(char::is_ascii_lowercase);

                // A single lowercase letter between two uppercase letters is an
                // abbreviation bridge when followed by more uppercase (DnDB → dnd-b).
                // MyAgent does NOT bridge because A is followed by lowercase g.
                let is_abbreviation_bridge = previous_was_lower_or_digit
                    && !next_is_lower
                    && index >= 2
                    && characters[index - 2].is_ascii_uppercase();

                if (previous_was_lower_or_digit && !is_abbreviation_bridge)
                    || (previous_was_upper && next_is_lower)
                {
                    raw.push('-');
                }
            }
            raw.push(character.to_ascii_lowercase());
        } else if character == ' ' || character == '_' {
            raw.push('-');
        } else {
            raw.push(character);
        }
    }

    // Collapse consecutive hyphens
    let mut collapsed = String::with_capacity(raw.len());
    let mut previous_was_hyphen = false;

    for character in raw.chars() {
        if character == '-' {
            if !previous_was_hyphen {
                collapsed.push('-');
            }
            previous_was_hyphen = true;
        } else {
            collapsed.push(character);
            previous_was_hyphen = false;
        }
    }

    collapsed
}
