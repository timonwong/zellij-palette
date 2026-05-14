use crate::model::PaletteItem;

const BOUNDARY_CHARS: [char; 8] = [' ', '-', '_', '·', '.', '/', ':', '\t'];

fn is_boundary(c: char) -> bool {
    BOUNDARY_CHARS.contains(&c)
}

fn auto_alias(title: &str) -> String {
    title
        .split(is_boundary)
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.chars().next())
        .collect::<String>()
        .to_lowercase()
}

fn exact_match_score(haystack: &str, needle: &str) -> usize {
    haystack.find(needle).map_or(0, |index| {
        let at_boundary = index == 0
            || haystack[..index]
                .chars()
                .next_back()
                .is_some_and(is_boundary);
        10_000 + usize::from(at_boundary) * 5_000 - index
    })
}

fn char_bonus(at_boundary: bool, consecutive: bool) -> usize {
    if at_boundary {
        50
    } else if consecutive {
        20
    } else {
        5
    }
}

fn subsequence_score(haystack: &str, needle: &str) -> usize {
    let haystack_chars: Vec<char> = haystack.chars().collect();
    let needle_chars: Vec<char> = needle.chars().collect();
    let mut score = 0;
    let mut haystack_index = 0usize;
    let mut previous_match: Option<usize> = None;

    for needle_char in needle_chars {
        while haystack_index < haystack_chars.len() && haystack_chars[haystack_index] != needle_char
        {
            haystack_index += 1;
        }

        if haystack_index == haystack_chars.len() {
            return 0;
        }

        let at_boundary =
            haystack_index == 0 || is_boundary(haystack_chars[haystack_index.saturating_sub(1)]);
        let consecutive = previous_match.is_some_and(|previous| haystack_index == previous + 1);
        score += char_bonus(at_boundary, consecutive);
        previous_match = Some(haystack_index);
        haystack_index += 1;
    }

    score.max(1)
}

fn fuzzy_score(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 1;
    }
    let haystack = haystack.to_lowercase();
    let needle = needle.to_lowercase();
    let exact = exact_match_score(&haystack, &needle);
    if exact > 0 {
        exact
    } else {
        subsequence_score(&haystack, &needle)
    }
}

fn item_haystack(item: &PaletteItem) -> String {
    let mut parts = vec![item.title.clone()];
    if let Some(description) = &item.description {
        parts.push(description.clone());
    }
    if let Some(category) = &item.category {
        parts.push(category.clone());
    }
    if let Some(shortcut) = &item.shortcut {
        parts.push(shortcut.clone());
    }
    parts.extend(item.aliases.iter().cloned());
    parts.push(auto_alias(&item.title));
    parts.join(" ")
}

pub fn filter_items(items: &[PaletteItem], query: &str) -> Vec<PaletteItem> {
    let parts: Vec<_> = query
        .split_whitespace()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return items.to_vec();
    }

    let mut matches: Vec<(usize, PaletteItem)> = items
        .iter()
        .filter_map(|item| {
            let haystack = item_haystack(item);
            let mut total_score = 0usize;
            for part in &parts {
                let score = fuzzy_score(&haystack, part);
                if score == 0 {
                    return None;
                }
                total_score += score;
            }
            Some((total_score.max(1), item.clone()))
        })
        .collect();

    matches.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.title.cmp(&right.1.title))
    });
    matches.into_iter().map(|(_, item)| item).collect()
}
