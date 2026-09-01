use std::cmp::Ordering;

pub const MAX_LYRICS_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_TIMED_CUES: usize = 20_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LyricsCue {
    pub start_ms: u64,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedLyrics {
    pub plain_text: String,
    pub cues: Vec<LyricsCue>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LyricsParseError {
    #[error("lyrics input exceeds the 2 MiB limit")]
    InputTooLarge,
    #[error("lyrics input is not valid UTF-8")]
    InvalidUtf8,
    #[error("lyrics contain more than {MAX_TIMED_CUES} timed cue lines")]
    TooManyCues,
    #[error("lyrics timestamp arithmetic overflowed")]
    TimestampOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TimedLine {
    start_ms: u64,
    order: usize,
}

/// Parse the small, deliberately bounded subset of LRC used by SpotDIY.
///
/// The parser treats malformed timing tags as ordinary visible text and never
/// uses floating point arithmetic, which keeps timestamp behavior deterministic.
pub fn parse_lrc(input: &str) -> Result<ParsedLyrics, LyricsParseError> {
    if input.len() > MAX_LYRICS_BYTES {
        return Err(LyricsParseError::InputTooLarge);
    }

    let mut offset_ms = 0_i64;
    let mut timed = Vec::<(TimedLine, String)>::new();
    let mut plain_lines = Vec::<String>::new();
    let mut timed_line_count = 0_usize;
    let mut order = 0_usize;

    for raw_line in input.trim_start_matches('\u{feff}').lines() {
        let line = raw_line.trim_end_matches('\r');
        if let Some(metadata) = metadata_tag(line) {
            if metadata.0.eq_ignore_ascii_case("offset") {
                if let Ok(value) = metadata.1.trim().parse::<i64>() {
                    offset_ms = value;
                }
            }
            continue;
        }

        let (timestamps, visible_start) = leading_timestamps(line)?;
        let visible = strip_inline_timing_markers(&line[visible_start..]);
        let visible = visible.trim_end().to_owned();
        if !timestamps.is_empty() {
            if timestamps.len() > MAX_TIMED_CUES.saturating_sub(timed_line_count) {
                return Err(LyricsParseError::TooManyCues);
            }
            timed_line_count += timestamps.len();
            if !visible.trim().is_empty() {
                plain_lines.push(visible.clone());
                for timestamp in timestamps {
                    let shifted = i128::from(timestamp)
                        .checked_add(i128::from(offset_ms))
                        .ok_or(LyricsParseError::TimestampOverflow)?;
                    let shifted = shifted.max(0);
                    let start_ms =
                        u64::try_from(shifted).map_err(|_| LyricsParseError::TimestampOverflow)?;
                    timed.push((TimedLine { start_ms, order }, visible.clone()));
                    order = order
                        .checked_add(1)
                        .ok_or(LyricsParseError::TimestampOverflow)?;
                }
            }
        } else if !visible.trim().is_empty() {
            plain_lines.push(visible);
        }
    }

    timed.sort_by(
        |(left, _), (right, _)| match left.start_ms.cmp(&right.start_ms) {
            Ordering::Equal => left.order.cmp(&right.order),
            ordering => ordering,
        },
    );

    let mut cues = Vec::<LyricsCue>::new();
    for (timed_line, text) in timed {
        if let Some(cue) = cues.last_mut() {
            if cue.start_ms == timed_line.start_ms {
                cue.lines.push(text);
                continue;
            }
        }
        cues.push(LyricsCue {
            start_ms: timed_line.start_ms,
            lines: vec![text],
        });
    }

    Ok(ParsedLyrics {
        plain_text: plain_lines.join("\n"),
        cues,
    })
}

pub fn parse_lrc_bytes(input: &[u8]) -> Result<ParsedLyrics, LyricsParseError> {
    if input.len() > MAX_LYRICS_BYTES {
        return Err(LyricsParseError::InputTooLarge);
    }
    let input = std::str::from_utf8(input).map_err(|_| LyricsParseError::InvalidUtf8)?;
    parse_lrc(input)
}

fn metadata_tag(line: &str) -> Option<(&str, &str)> {
    if !line.starts_with('[') {
        return None;
    }
    let end = line.find(']')?;
    let body = &line[1..end];
    let (key, value) = body.split_once(':')?;
    let key = key.trim();
    if matches!(
        key.to_ascii_lowercase().as_str(),
        "ar" | "ti" | "al" | "by" | "re" | "ve" | "length" | "offset"
    ) {
        Some((key, value))
    } else {
        None
    }
}

fn leading_timestamps(line: &str) -> Result<(Vec<u64>, usize), LyricsParseError> {
    let mut timestamps = Vec::new();
    let mut cursor = 0;
    while line.as_bytes().get(cursor) == Some(&b'[') {
        let Some(relative_end) = line[cursor + 1..].find(']') else {
            break;
        };
        let end = cursor + 1 + relative_end;
        let body = &line[cursor + 1..end];
        let Some(timestamp) = parse_timestamp(body)? else {
            break;
        };
        timestamps.push(timestamp);
        cursor = end + 1;
    }
    Ok((timestamps, cursor))
}

fn parse_timestamp(value: &str) -> Result<Option<u64>, LyricsParseError> {
    let Some((minutes, fraction)) = value.split_once(':') else {
        return Ok(None);
    };
    if minutes.is_empty() || !minutes.bytes().all(|byte| byte.is_ascii_digit()) {
        return Ok(None);
    }
    let (seconds, fraction) = match fraction.split_once('.') {
        Some((seconds, fraction)) => (seconds, Some(fraction)),
        None => (fraction, None),
    };
    if seconds.len() != 2 || !seconds.bytes().all(|byte| byte.is_ascii_digit()) {
        return Ok(None);
    }
    let seconds = seconds
        .parse::<u64>()
        .map_err(|_| LyricsParseError::TimestampOverflow)?;
    if seconds >= 60 {
        return Ok(None);
    }
    let fraction_ms = match fraction {
        None => 0,
        Some(value)
            if (1..=3).contains(&value.len())
                && value.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            let parsed = value
                .parse::<u64>()
                .map_err(|_| LyricsParseError::TimestampOverflow)?;
            match value.len() {
                1 => parsed
                    .checked_mul(100)
                    .ok_or(LyricsParseError::TimestampOverflow)?,
                2 => parsed
                    .checked_mul(10)
                    .ok_or(LyricsParseError::TimestampOverflow)?,
                _ => parsed,
            }
        }
        Some(_) => return Ok(None),
    };
    let minutes = minutes
        .parse::<u64>()
        .map_err(|_| LyricsParseError::TimestampOverflow)?;
    minutes
        .checked_mul(60_000)
        .ok_or(LyricsParseError::TimestampOverflow)?
        .checked_add(
            seconds
                .checked_mul(1_000)
                .ok_or(LyricsParseError::TimestampOverflow)?,
        )
        .ok_or(LyricsParseError::TimestampOverflow)?
        .checked_add(fraction_ms)
        .ok_or(LyricsParseError::TimestampOverflow)
        .map(Some)
}

fn strip_inline_timing_markers(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    while cursor < value.len() {
        let remaining = &value[cursor..];
        if remaining.as_bytes().first() == Some(&b'<') {
            if let Some(relative_end) = remaining.find('>') {
                let body = &remaining[1..relative_end];
                if parse_timestamp(body).is_ok_and(|value| value.is_some()) {
                    cursor += relative_end + 1;
                    continue;
                }
            }
        }
        let character = remaining
            .chars()
            .next()
            .expect("cursor is always on a character boundary");
        output.push(character);
        cursor += character.len_utf8();
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fraction_precision_and_groups_stably() {
        let parsed =
            parse_lrc("[00:02.5]later\n[00:01]first\n[00:02.50]same\n[00:02.500]third").unwrap();
        assert_eq!(
            parsed.cues,
            vec![
                LyricsCue {
                    start_ms: 1_000,
                    lines: vec!["first".to_owned()]
                },
                LyricsCue {
                    start_ms: 2_500,
                    lines: vec!["later".to_owned(), "same".to_owned(), "third".to_owned()]
                },
            ]
        );
    }

    #[test]
    fn metadata_offsets_and_inline_markers_are_not_visible() {
        let parsed = parse_lrc("\u{feff}[ar:Synthetic]\r\n[offset:-1500]\r\n[00:01.00]one <00:01.20>word\r\n[00:02.00]two").unwrap();
        assert_eq!(parsed.cues[0].start_ms, 0);
        assert_eq!(parsed.cues[0].lines[0], "one word");
        assert_eq!(parsed.plain_text, "one word\ntwo");
    }

    #[test]
    fn malformed_timestamps_fall_back_to_plain_text() {
        let parsed = parse_lrc("[bad]visible\n[00:60]also visible").unwrap();
        assert!(parsed.cues.is_empty());
        assert_eq!(parsed.plain_text, "[bad]visible\n[00:60]also visible");
    }

    #[test]
    fn rejects_oversized_input_and_too_many_timed_lines() {
        assert_eq!(
            parse_lrc(&"x".repeat(MAX_LYRICS_BYTES + 1)),
            Err(LyricsParseError::InputTooLarge)
        );
        let input = (0..=MAX_TIMED_CUES)
            .map(|index| format!("[00:{:02}]line", index % 60))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(parse_lrc(&input), Err(LyricsParseError::TooManyCues));
    }

    #[test]
    fn rejects_timestamp_arithmetic_overflow() {
        assert_eq!(
            parse_lrc("[18446744073709551615:59]overflow"),
            Err(LyricsParseError::TimestampOverflow)
        );
    }
}
