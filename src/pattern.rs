extern crate nom;

use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::space0,
    combinator::{all_consuming, opt, verify},
    multi::fold_many1,
    number::complete::float,
    IResult,
};
use std::{
    collections::HashMap,
    fmt,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use crate::{
    error::{
        Error::*,
        Result
    },
    audio::Tracks,
    Instrumentation,
    steps::Steps,
};

/// Indicates a *play* step.
const STEP_PLAY: &str = "x";

/// Indicates a *silent* step.
const STEP_SILENT: &str = "-";

/// The beat separator in a step sequence.
const SEPARATOR: &str = "|";

/// The notes
const NOTE_A:  &str = "A";
const NOTE_AS: &str = "A#";
const NOTE_BF: &str = "Bb";
const NOTE_B:  &str = "B";
const NOTE_C:  &str = "C";
const NOTE_CS: &str = "C#";
const NOTE_DF: &str = "Db";
const NOTE_D:  &str = "D";
const NOTE_DS: &str = "D#";
const NOTE_EF: &str = "Eb";
const NOTE_E:  &str = "E";
const NOTE_F:  &str = "F";
const NOTE_FS: &str = "F#";
const NOTE_GF: &str = "Gb";
const NOTE_G:  &str = "G";
const NOTE_GS: &str = "G#";
const NOTE_AF: &str = "Ab";

/// Reperesents the contents of a pattern file.
///
/// Each line of a pattern file represents a track. There is no limit to the number
/// of tracks in a pattern. A track contains an instrument name, a 16-step sequence,
/// and an optional amplitude. The instrument name is an identifier and can only
/// appear once per pattern. Each sequence represents a single measure in 4/4 time
/// divided into 16th note steps (`x` for *play* and `-` for *silent*).
/// A track may optionally include an amplitude in the range of [0,1] inclusive.
/// By default, a track plays at full volume.
///
/// # Example
///
/// This is an example of a pattern file's contents for a standard 8th note groove
/// with the hi-hat track played at half volume.
///
/// ```text
/// hi-hat |x-x-|x-x-|x-x-|x-x-| 0.5
/// snare  |----|x---|----|x---|
/// kick   |x---|----|x---|----|
/// ```
#[derive(Debug)]
pub struct Pattern(HashMap<Instrument, (Steps, Amplitude)>);

impl Pattern {
    /// Parses a pattern file located at the path given.
    pub fn parse(p: &Path) -> Result<Pattern> {
        if !p.is_file() {
            return Err(FileDoesNotExistError(p.into()));
        }
        let f = File::open(p)?;
        let r = BufReader::new(f);

        let mut m: HashMap<Instrument, (Steps, Amplitude)> = HashMap::new();
        for l in r.lines() {
            let l = l?;
            match parse_track(&l[..]) {
                Ok((_, (i, s, a))) => match m.insert(i, (s, a)) {
                    Some(_) => return Err(DuplicatePatternError(l)),
                    None => (),
                },
                _ => return Err(ParseError(l)),
            }
        }

        Ok(Pattern(m))
    }

    /// Returns the step sequence and amplitide associated with the instrument given.
    pub fn get(&self, i: &Instrument) -> Option<&(Steps, Amplitude)> {
        self.0.get(i)
    }

    pub fn len(&self) -> usize {
        let mut max_len: usize = 0;
        for (_, (s, _)) in self.0.iter() {
            if s.len() > max_len {
                max_len = s.len();
            }
        }
        max_len
    }

    /// Binds a pattern's step sequences to audio files.
    /// Any sequences bound to the same audio file will be unioned.
    /// The smallest amplitude for instruments bound to the same audio file will be used.
    pub fn bind(&self, instrumentation: Instrumentation) -> Tracks {
        let mut aggregate_steps = Steps::zeros(self.len());
        Tracks::from(
            instrumentation
                .into_iter()
                .map(|(sample_file, instruments)| {
                    let simplified_steps = instruments.iter().fold(
                        (Steps::zeros(self.len()), Amplitude::max()),
                        |mut acc, instrument| {
                            if let Some((steps, amplitude)) = self.get(instrument) {
                                // update the aggregate step sequence
                                aggregate_steps = aggregate_steps.union(steps);

                                // update the track's step sequence and amplitude
                                acc.0 = acc.0.union(steps);
                                acc.1 = acc.1.min(amplitude);
                            }

                            acc
                        },
                    );

                    (sample_file, simplified_steps)
                })
                .collect()
        )
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, (s, a)) in self.0.iter() {
            writeln!(f, "{} {} {}", i, s, a)?;
        }

        Ok(())
    }
}

/// Represents a track's instrument name.
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct Instrument(String);

impl From<&str> for Instrument {
    #[inline]
    fn from(s: &str) -> Instrument {
        Instrument(String::from(s))
    }
}

impl fmt::Display for Instrument {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents a track's amplitude in the range of [0,1] inclusive.
#[derive(Debug, Clone)]
pub struct Amplitude(f32);

impl Amplitude {
    /// Returns an amplitude of the maximum value.
    pub fn max() -> Amplitude {
        Amplitude(1.0)
    }

    /// Compares the amplitude to another and returns the minimum.
    pub fn min(&self, other: &Amplitude) -> Amplitude {
        Amplitude(self.0.min(other.0))
    }

    /// Returns the amplitude's value.
    pub fn value(&self) -> f32 {
        self.0
    }

    fn defaulting(o: Option<f32>) -> Amplitude {
        Amplitude(o.unwrap_or(1.0))
    }
}

impl fmt::Display for Amplitude {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A type that represents a track in a pattern file.
type Track = (Instrument, Steps, Amplitude);

/// Parses a track from a single line of a pattern file.
fn parse_track(s: &str) -> IResult<&str, Track> {
    let (s, _) = space0(s)?;
    let (s, instrument) = parse_instrument(s)?;
    let (s, _) = space0(s)?;
    let (s, steps) = parse_steps(s)?;
    let (s, _) = space0(s)?;
    let (s, amplitude) = parse_amplitude(s)?;
    let (s, _) = all_consuming(space0)(s)?;

    Ok((
        s,
        (
            Instrument::from(instrument),
            steps,
            Amplitude::defaulting(amplitude),
        ),
    ))
}

/// Parses the instrument from a track line.
fn parse_instrument(s: &str) -> IResult<&str, &str> {
    is_not(" \t")(s)
}

/// Parses the steps from a track line.
fn parse_steps(s: &str) -> IResult<&str, Steps> {
    let p = fold_many1(
        alt((tag(STEP_PLAY), tag(STEP_SILENT), tag(SEPARATOR), tag(NOTE_A), tag(NOTE_B), tag(NOTE_C), tag(NOTE_D))),
        || Steps::new(),
        |mut acc: Steps, i| {
            match i {
                STEP_PLAY => acc.push(255, 440.0),
                STEP_SILENT => acc.push(0, 0.0),
                NOTE_A => acc.push(255, 440.0),
                NOTE_B => acc.push(255, 493.88),
                NOTE_C => acc.push(255, 523.25),
                NOTE_D => acc.push(0x3f, 587.33),
                _ => (),
            }
            acc
        },
    );

    verify(p, |v: &Steps| v.len() > 0)(s)
}

/// Parses the amplitude from a track line.
fn parse_amplitude(s: &str) -> IResult<&str, Option<f32>> {
    verify(opt(float), |o: &Option<f32>| match *o {
        Some(v) => 0.0 <= v && v <= 1.0,
        None => true,
    })(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitvec::{bitvec, prelude::Lsb0};

    #[test]
    fn test_parse_track() {
        let s = "a |----|----|----|----|";
        let p = parse_track(s).unwrap();
        let r = p.0;
        let l = p.1;

        assert_eq!(r, "");
        assert_eq!(l.0, Instrument::from("a"));
        assert_eq!(l.1, Steps::from(bitvec![0; 16]));
    }

    #[test]
    fn test_parse_instrument() {
        let s1 = "";
        let s2 = "a";
        let s3 = "a ";
        let s4 = "a  ";
        let s5 = "a\t";
        let s6 = "a \t";

        assert!(parse_instrument(s1).is_err());
        assert_eq!(parse_instrument(s2).unwrap(), ("", "a"));
        assert_eq!(parse_instrument(s3).unwrap(), (" ", "a"));
        assert_eq!(parse_instrument(s4).unwrap(), ("  ", "a"));
        assert_eq!(parse_instrument(s5).unwrap(), ("\t", "a"));
        assert_eq!(parse_instrument(s6).unwrap(), (" \t", "a"));
    }

    #[test]
    fn test_parse_steps() {
        let s1 = "";
        let s2 = "|----|";
        let s3 = "|----|----|----|----|-";
        let s4 = "|----|----|----|----|";
        let s5 = "|xxxx|xxxx|xxxx|xxxx|";
        let s6 = "|x-x-|x-x-|x-x-|x-x-|";

        assert!(parse_steps(s1).is_err());
        assert_eq!(
            parse_steps(s2).unwrap(),
            ("", Steps::from(bitvec![0; 4]))
        );
        assert_eq!(
            parse_steps(s3).unwrap(),
            ("", Steps::from(bitvec![0; 17]))
        );
        assert_eq!(
            parse_steps(s4).unwrap(),
            ("", Steps::from(bitvec![0; 16]))
        );
        assert_eq!(
            parse_steps(s5).unwrap(),
            ("", Steps::from(bitvec![1; 16]))
        );
        assert_eq!(
            parse_steps(s6).unwrap(),
            ("", Steps::from(bitvec![1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0]))
        );
    }

    #[test]
    fn test_parse_amplitude() {
        let s1 = "";
        let s2 = "abc";
        let s3 = "0.0";
        let s4 = "0.5";
        let s5 = "1.0";
        let s6 = "-1.0";
        let s7 = "1.1";

        assert_eq!(parse_amplitude(s1).unwrap(), ("", None));
        assert_eq!(parse_amplitude(s2).unwrap(), (s2, None));
        assert_eq!(parse_amplitude(s3).unwrap(), ("", Some(0.0)));
        assert_eq!(parse_amplitude(s4).unwrap(), ("", Some(0.5)));
        assert_eq!(parse_amplitude(s5).unwrap(), ("", Some(1.0)));
        assert!(parse_amplitude(s6).is_err());
        assert!(parse_amplitude(s7).is_err());
    }
}
