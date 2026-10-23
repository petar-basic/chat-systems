const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

fn digit(byte: u8) -> usize {
    DIGITS.iter().position(|d| *d == byte).unwrap_or(0)
}

pub fn between(before: Option<&str>, after: Option<&str>) -> String {
    midpoint(before.unwrap_or("").as_bytes(), after.map(str::as_bytes))
}

fn midpoint(low: &[u8], high: Option<&[u8]>) -> String {
    if let Some(high) = high {
        let shared = high
            .iter()
            .enumerate()
            .take_while(|(i, byte)| low.get(*i).copied().unwrap_or(b'0') == **byte)
            .count();
        if shared > 0 {
            let prefix = String::from_utf8_lossy(&high[..shared]).into_owned();
            let rest_low = low.get(shared..).unwrap_or(&[]);
            return prefix + &midpoint(rest_low, Some(&high[shared..]));
        }
    }
    let low_digit = low.first().map_or(0, |b| digit(*b));
    let high_digit = high
        .and_then(|h| h.first())
        .map_or(DIGITS.len(), |b| digit(*b));
    if high_digit - low_digit > 1 {
        let mid = (low_digit + high_digit).div_ceil(2);
        return (DIGITS[mid] as char).to_string();
    }
    if let Some(high) = high.filter(|h| h.len() > 1) {
        return (high[0] as char).to_string();
    }
    let rest_low = low.get(1..).unwrap_or(&[]);
    (DIGITS[low_digit] as char).to_string() + &midpoint(rest_low, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ordered(low: Option<&str>, high: Option<&str>) -> String {
        let mid = between(low, high);
        if let Some(low) = low {
            assert!(low < mid.as_str(), "{low} < {mid}");
        }
        if let Some(high) = high {
            assert!(mid.as_str() < high, "{mid} < {high}");
        }
        assert!(!mid.ends_with('0'), "{mid} has a trailing zero");
        mid
    }

    #[test]
    fn a_rank_falls_between_its_neighbours() {
        assert_eq!(ordered(None, None), "V");
        ordered(Some("V"), None);
        ordered(None, Some("V"));
        ordered(Some("a"), Some("b"));
        ordered(Some("a0V"), Some("a1"));
        ordered(Some("z"), None);
        ordered(None, Some("01"));
        ordered(Some("az"), Some("b"));
    }

    #[test]
    fn appending_and_prepending_keep_going() {
        let mut last = between(None, None);
        for _ in 0..500 {
            last = ordered(Some(&last), None);
        }
        let mut first = between(None, None);
        for _ in 0..500 {
            first = ordered(None, Some(&first));
        }
    }

    #[test]
    fn squeezing_into_one_gap_keeps_going() {
        let low = "V".to_string();
        let mut high = between(Some(&low), None);
        for _ in 0..200 {
            high = ordered(Some(&low), Some(&high));
        }
    }
}
