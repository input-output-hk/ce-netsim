use anyhow::{anyhow, bail, ensure, Result};
use core::fmt;
use logos::{Lexer, Logos};
use std::{str::FromStr, time};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub(crate) struct Duration(std::time::Duration);

impl Duration {
    pub(crate) fn new(dur: std::time::Duration) -> Self {
        Self(dur)
    }

    #[inline]
    pub fn into_duration(self) -> time::Duration {
        self.0
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_zero() {
            return write!(f, "0ms");
        }

        let subsec_nanos = self.0.subsec_nanos();
        let seconds = self.0.as_secs();

        let nanos = subsec_nanos % 1_000;
        let micros = (subsec_nanos / 1_000) % 1_000;
        let millis = (subsec_nanos / 1_000_000) % 1_000;
        let secs = seconds % 60;
        let minutes = seconds / 60;

        if minutes > 0 {
            write!(f, "{minutes}m")?;
        }
        if secs > 0 {
            write!(f, "{secs}s")?;
        }
        if millis > 0 {
            write!(f, "{millis}ms")?;
        }
        if micros > 0 {
            write!(f, "{micros}µs")?;
        }
        if nanos > 0 {
            write!(f, "{nanos}ns")?;
        }

        Ok(())
    }
}

impl FromStr for Duration {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut lex = Lexer::new(s);

        let mut durations = Vec::new();

        while let Some(next) = lex.next() {
            let number: Token = next.map_err(|()| anyhow!("Failed to parse: {s}"))?;

            ensure!(
                number == Token::Value,
                "Expecting duration to starts with number. Cannot parse {s}"
            );
            let number: u64 = lex.slice().parse()?;

            let Some(Ok(measure)) = lex.next() else {
                bail!("Expecting a measure, failed to parse: {s}")
            };
            let duration = match measure {
                Token::NanoSeconds => time::Duration::from_nanos(number),
                Token::MicroSeconds => time::Duration::from_micros(number),
                Token::MilliSeconds => time::Duration::from_millis(number),
                Token::Seconds => time::Duration::from_secs(number),
                Token::Minutes => time::Duration::from_secs(number * 60),
                Token::Value => bail!("Failed to parse `{s}', expecting a measure."),
            };
            durations.push(duration);
        }

        Ok(Self(durations.into_iter().sum()))
    }
}

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[ \t\n\f]+")] // Ignore this regex pattern between tokens
enum Token {
    #[token("ns")]
    NanoSeconds,
    #[regex("us|μs|µs")]
    MicroSeconds,
    #[token("ms")]
    MilliSeconds,
    #[token("s")]
    Seconds,
    #[token("m")]
    Minutes,

    #[regex("[0-9]+")]
    Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logos_lexer() {
        let mut lex = Token::lexer("1ns");

        assert_eq!(lex.next(), Some(Ok(Token::Value)));
        assert_eq!(lex.span(), 0..1);
        assert_eq!(lex.slice(), "1");

        assert_eq!(lex.next(), Some(Ok(Token::NanoSeconds)));
        assert_eq!(lex.span(), 1..3);
        assert_eq!(lex.slice(), "ns");
    }

    #[test]
    fn parse() {
        let Duration(duration) = "0ms".parse().unwrap();
        assert_eq!(duration.as_millis(), 0);

        let Duration(duration) = "123ms".parse().unwrap();
        assert_eq!(duration.as_millis(), 123);

        let Duration(duration) = "1s 2000ms 3000000us".parse().unwrap();
        assert_eq!(duration.as_secs(), 6);
    }

    #[test]
    fn display() {
        assert_eq!(Duration::new(std::time::Duration::ZERO).to_string(), "0ms");

        assert_eq!(
            Duration::new(std::time::Duration::from_secs(5 * 60)).to_string(),
            "5m"
        );
        assert_eq!(
            Duration::new(std::time::Duration::from_secs(5)).to_string(),
            "5s"
        );
        assert_eq!(
            Duration::new(std::time::Duration::from_millis(5)).to_string(),
            "5ms"
        );
        assert_eq!(
            Duration::new(std::time::Duration::from_micros(5)).to_string(),
            "5µs"
        );
        assert_eq!(
            Duration::new(std::time::Duration::from_nanos(5)).to_string(),
            "5ns"
        );

        assert_eq!(
            Duration::new(
                std::time::Duration::from_secs(5 * 60)
                    + std::time::Duration::from_secs(42)
                    + std::time::Duration::from_millis(999)
                    + std::time::Duration::from_micros(123)
                    + std::time::Duration::from_nanos(1)
            )
            .to_string(),
            "5m42s999ms123µs1ns"
        );
    }

    /// test that what is displayed can be parsed too
    #[test]
    fn display_parse() {
        let duration = Duration::new(
            std::time::Duration::from_secs(120)
                + std::time::Duration::from_secs(1)
                + std::time::Duration::from_millis(42)
                + std::time::Duration::from_nanos(999),
        );

        let displayed = duration.to_string();
        let parsed: Duration = displayed.parse().unwrap();

        assert_eq!(parsed, duration);
    }
}
