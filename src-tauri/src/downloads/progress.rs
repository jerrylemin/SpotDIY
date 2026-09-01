use std::path::PathBuf;

pub const PROGRESS_PREFIX: &str = "SPOTDIY_PROGRESS";
pub const FILE_PREFIX: &str = "SPOTDIY_FILE";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadProgressUpdate {
    pub status: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub total_bytes_estimate: Option<u64>,
    pub speed_bytes_per_second: Option<u64>,
    pub eta_seconds: Option<u64>,
}

impl DownloadProgressUpdate {
    pub fn progress_permille(&self) -> u16 {
        let total = self.total_bytes.or(self.total_bytes_estimate);
        let Some(total) = total.filter(|value| *value > 0) else {
            return 0;
        };
        let progress = self
            .downloaded_bytes
            .saturating_mul(1000)
            .checked_div(total)
            .unwrap_or(0);
        progress.min(1000) as u16
    }
}

pub fn parse_progress_line(line: &str) -> Option<DownloadProgressUpdate> {
    let mut fields = line.split('\t');
    if fields.next()? != PROGRESS_PREFIX {
        return None;
    }
    let status = fields.next()?.trim();
    if status.is_empty() {
        return None;
    }
    let downloaded_bytes = parse_required_integer(fields.next()?)?;
    let total_bytes = parse_optional_integer(fields.next()?);
    let total_bytes_estimate = parse_optional_integer(fields.next()?);
    let speed_bytes_per_second = parse_optional_number(fields.next()?);
    let eta_seconds = parse_optional_integer(fields.next()?);
    if fields.next().is_some() {
        return None;
    }
    Some(DownloadProgressUpdate {
        status: status.to_owned(),
        downloaded_bytes,
        total_bytes,
        total_bytes_estimate,
        speed_bytes_per_second,
        eta_seconds,
    })
}

pub fn parse_file_line(line: &str) -> Option<PathBuf> {
    let value = line.strip_prefix(FILE_PREFIX)?.strip_prefix('\t')?;
    if value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    Some(PathBuf::from(value))
}

fn parse_required_integer(value: &str) -> Option<u64> {
    if is_unknown(value) {
        return None;
    }
    value.parse::<u64>().ok()
}

fn parse_optional_integer(value: &str) -> Option<u64> {
    if is_unknown(value) {
        return None;
    }
    value.parse::<u64>().ok()
}

fn parse_optional_number(value: &str) -> Option<u64> {
    if is_unknown(value) {
        return None;
    }
    let value = value.trim();
    if value.starts_with('-') {
        return None;
    }
    if let Ok(integer) = value.parse::<u64>() {
        return Some(integer);
    }
    let decimal = value.parse::<f64>().ok()?;
    (decimal.is_finite() && decimal >= 0.0 && decimal <= u64::MAX as f64)
        .then_some(decimal.floor() as u64)
}

fn is_unknown(value: &str) -> bool {
    matches!(
        value.trim(),
        "" | "NA" | "N/A" | "None" | "none" | "null" | "NULL"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_machine_progress_and_clamps_progress() {
        let progress =
            parse_progress_line("SPOTDIY_PROGRESS\tdownloading\t125\t100\tNA\t12.8\t3").unwrap();
        assert_eq!(progress.downloaded_bytes, 125);
        assert_eq!(progress.total_bytes, Some(100));
        assert_eq!(progress.total_bytes_estimate, None);
        assert_eq!(progress.speed_bytes_per_second, Some(12));
        assert_eq!(progress.progress_permille(), 1000);
    }

    #[test]
    fn unknown_progress_values_are_none() {
        let progress =
            parse_progress_line("SPOTDIY_PROGRESS\tdownloading\t0\tNA\tN/A\tNone\tNULL").unwrap();
        assert_eq!(progress.total_bytes, None);
        assert_eq!(progress.total_bytes_estimate, None);
        assert_eq!(progress.speed_bytes_per_second, None);
        assert_eq!(progress.eta_seconds, None);
    }

    #[test]
    fn malformed_or_human_progress_does_not_parse() {
        for line in [
            "[download] 10% of 20MiB",
            "SPOTDIY_PROGRESS\tdownloading\t-1\t100\tNA\t1\t1",
            "SPOTDIY_PROGRESS\tdownloading\t1\t100\tNA\t1\t1\textra",
        ] {
            assert_eq!(parse_progress_line(line), None);
        }
    }

    #[test]
    fn file_record_rejects_control_characters() {
        assert_eq!(
            parse_file_line("SPOTDIY_FILE\tC:\\temp\\media.webm"),
            Some(PathBuf::from("C:\\temp\\media.webm"))
        );
        assert_eq!(parse_file_line("SPOTDIY_FILE\tC:\\temp\nmedia.webm"), None);
    }
}
