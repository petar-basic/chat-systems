use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteRange {
    From(u64),
    Bounded { first: u64, last: u64 },
    Suffix(u64),
}

impl ByteRange {
    pub fn parse(header: &str) -> Option<Self> {
        let spec = header.trim().strip_prefix("bytes=")?.trim();
        if spec.contains(',') {
            return None;
        }
        let (first, last) = spec.split_once('-')?;
        match (first.trim(), last.trim()) {
            ("", "") => None,
            ("", suffix) => suffix.parse().ok().map(ByteRange::Suffix),
            (first, "") => first.parse().ok().map(ByteRange::From),
            (first, last) => {
                let (first, last): (u64, u64) = (first.parse().ok()?, last.parse().ok()?);
                (first <= last).then_some(ByteRange::Bounded { first, last })
            }
        }
    }

    pub fn resolve(self, total: u64) -> Option<Range<u64>> {
        match self {
            ByteRange::From(first) if first < total => Some(first..total),
            ByteRange::Bounded { first, last } if first < total => {
                Some(first..last.saturating_add(1).min(total))
            }
            ByteRange::Suffix(length) if length > 0 && total > 0 => {
                Some(total.saturating_sub(length)..total)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ByteRange;

    #[test]
    fn reads_the_three_single_range_forms() {
        assert_eq!(
            ByteRange::parse("bytes=2-5"),
            Some(ByteRange::Bounded { first: 2, last: 5 })
        );
        assert_eq!(ByteRange::parse("bytes=7-"), Some(ByteRange::From(7)));
        assert_eq!(ByteRange::parse("bytes=-3"), Some(ByteRange::Suffix(3)));
    }

    #[test]
    fn ignores_what_it_does_not_serve() {
        for header in [
            "bytes=0-1,4-5",
            "bytes=5-2",
            "bytes=-",
            "items=0-1",
            "bytes=a-b",
        ] {
            assert_eq!(ByteRange::parse(header), None, "{header}");
        }
    }

    #[test]
    fn clamps_to_the_object_and_refuses_what_lies_past_it() {
        assert_eq!(
            ByteRange::Bounded { first: 2, last: 5 }.resolve(10),
            Some(2..6)
        );
        assert_eq!(
            ByteRange::Bounded { first: 8, last: 50 }.resolve(10),
            Some(8..10)
        );
        assert_eq!(ByteRange::From(7).resolve(10), Some(7..10));
        assert_eq!(ByteRange::Suffix(3).resolve(10), Some(7..10));
        assert_eq!(ByteRange::Suffix(30).resolve(10), Some(0..10));
        assert_eq!(ByteRange::From(10).resolve(10), None);
        assert_eq!(ByteRange::Suffix(0).resolve(10), None);
        assert_eq!(ByteRange::From(0).resolve(0), None);
    }
}
