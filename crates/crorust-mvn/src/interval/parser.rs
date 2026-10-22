use crate::interval::constraint::{IntervalBoundEndpoint, VersionConstraint};
use crate::interval::lexer::{IntervalLexer, IntervalToken};
use crate::model::coordinate::SemverVersion;
use anyhow::Result;
use pubgrub::range::Range as SRange;
use pubgrub::version::Version as _;
use std::str::FromStr;

pub struct IntervalParser<I> {
    iter: I,
    first: bool,
}

impl<I: Iterator<Item = IntervalToken>> IntervalParser<I> {
    pub fn new(iter: I) -> Self {
        Self { iter, first: true }
    }

    pub fn parse_range(&mut self) -> Option<Result<VersionConstraint>> {
        let first_token = self.iter.next()?;
        match first_token {
            IntervalToken::Version(version) => {
                Some(Ok(VersionConstraint::Greater(IntervalBoundEndpoint {
                    version,
                    inclusive: true,
                })))
            }
            IntervalToken::Open(inclusive) => {
                let lower_token = match self.iter.next() {
                    Some(t) => t,
                    None => {
                        return Some(Err(anyhow::anyhow!(
                            "Unexpected EOF after opening range delimiter"
                        )))
                    }
                };
                let lower_bound = match lower_token {
                    IntervalToken::Version(version) => match self.iter.next() {
                        Some(IntervalToken::Comma) => {
                            Some(IntervalBoundEndpoint { version, inclusive })
                        }
                        Some(IntervalToken::Close(true)) if inclusive => {
                            return Some(Ok(VersionConstraint::Exact(version)))
                        }
                        other => {
                            return Some(Err(anyhow::anyhow!(
                                "Unexpected token {:?} in lower bound",
                                other
                            )))
                        }
                    },
                    IntervalToken::Comma => None,
                    other => {
                        return Some(Err(anyhow::anyhow!(
                            "Unexpected token {:?} in lower bound",
                            other
                        )))
                    }
                };
                let upper_token = match self.iter.next() {
                    Some(t) => t,
                    None => {
                        return Some(Err(anyhow::anyhow!("Unexpected EOF in range upper bound")))
                    }
                };
                let upper_bound = match upper_token {
                    IntervalToken::Close(_) => None,
                    IntervalToken::Version(version) => match self.iter.next() {
                        Some(IntervalToken::Close(inclusive)) => {
                            Some(IntervalBoundEndpoint { version, inclusive })
                        }
                        other => {
                            return Some(Err(anyhow::anyhow!(
                                "Unexpected token {:?} after upper bound version",
                                other
                            )))
                        }
                    },
                    other => {
                        return Some(Err(anyhow::anyhow!(
                            "Unexpected token {:?} in upper bound",
                            other
                        )))
                    }
                };
                let range = match (lower_bound, upper_bound) {
                    (None, Some(bound)) => VersionConstraint::Lower(bound),
                    (Some(bound), None) => VersionConstraint::Greater(bound),
                    (Some(lower), Some(upper)) => VersionConstraint::Between(lower, upper),
                    (None, None) => {
                        return Some(Err(anyhow::anyhow!("Empty range bounds without versions")))
                    }
                };
                Some(Ok(range))
            }
            other => Some(Err(anyhow::anyhow!(
                "Unexpected token {:?} at start of range",
                other
            ))),
        }
    }
}

impl<I: Iterator<Item = IntervalToken>> Iterator for IntervalParser<I> {
    type Item = Result<VersionConstraint>;

    fn next(&mut self) -> Option<Result<VersionConstraint>> {
        if !self.first {
            match self.iter.next()? {
                IntervalToken::Comma => {}
                other => {
                    return Some(Err(anyhow::anyhow!(
                        "Expected ',' between Maven version range intervals, found {:?}",
                        other
                    )));
                }
            }
        }
        self.first = false;
        self.parse_range()
    }
}

pub fn parse_version_constraint(range_str: &str) -> Result<SRange<SemverVersion>> {
    let trimmed = range_str.trim();
    anyhow::ensure!(!trimmed.is_empty(), "Maven version range cannot be empty");

    let mut parser = IntervalParser::new(IntervalLexer::new(trimmed.chars()));
    let mut range = SRange::none();
    let mut parsed_any = false;

    for partial_res in &mut parser {
        let partial_range = partial_res?;
        parsed_any = true;
        let srange = match partial_range {
            VersionConstraint::Exact(version) => SRange::exact(SemverVersion::from_str(&version)?),
            VersionConstraint::Lower(IntervalBoundEndpoint { version, inclusive }) => {
                let mut version = SemverVersion::from_str(&version)?;
                if inclusive {
                    version = version.bump();
                }
                SRange::strictly_lower_than(version)
            }
            VersionConstraint::Greater(IntervalBoundEndpoint { version, inclusive }) => {
                let mut version = SemverVersion::from_str(&version)?;
                if !inclusive {
                    version = version.bump();
                }
                SRange::higher_than(version)
            }
            VersionConstraint::Between(lower_bound, upper_bound) => {
                let mut lower_version = SemverVersion::from_str(&lower_bound.version)?;
                if !lower_bound.inclusive {
                    lower_version = lower_version.bump();
                }
                let mut upper_version = SemverVersion::from_str(&upper_bound.version)?;
                if upper_bound.inclusive {
                    upper_version = upper_version.bump();
                }
                SRange::between(lower_version, upper_version)
            }
        };
        range = range.union(&srange);
    }

    anyhow::ensure!(
        parsed_any,
        "Malformed Maven version range expression `{}`",
        range_str
    );

    Ok(range)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_range_evaluations() -> Result<()> {
        let upper_closed = parse_version_constraint("(,1.0]")?;
        assert!(upper_closed.contains(&"1.0".parse()?));
        assert!(upper_closed.contains(&"0.9.5".parse()?));
        assert!(!upper_closed.contains(&"1.0.1".parse()?));

        let soft_floor = parse_version_constraint("1.0")?;
        assert!(soft_floor.contains(&"1.0".parse()?));
        assert!(soft_floor.contains(&"2.0".parse()?));
        assert!(!soft_floor.contains(&"0.9".parse()?));

        let exact_pinned = parse_version_constraint("[1.0]")?;
        assert!(exact_pinned.contains(&"1.0".parse()?));
        assert!(!exact_pinned.contains(&"1.0.1".parse()?));
        assert!(!exact_pinned.contains(&"0.9.9".parse()?));
        Ok(())
    }
}
