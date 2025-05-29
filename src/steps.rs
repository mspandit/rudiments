use std::{
    fmt,
};

#[derive(Debug, PartialEq, Clone)]
pub struct Steps(Vec<(u8, f32)>); // MIDI velocity, frequency

impl Steps {
    pub fn new() -> Steps {
        Steps(Vec::new())
    }

    pub fn push(&mut self, vel: u8, freq: f32) {
        self.0.push((vel, freq))
    }

    pub fn zeros(steps: usize) -> Steps {
        Steps(vec![(0, 0.0); steps])
    }

    pub fn union(&mut self, other: &Steps) -> Steps {
        Steps(
            self
                .iter()
                .zip(other.iter())
                .map(|((v1, f1), (v2, f2))| { if *v1 > *v2 { (*v1, *f1) } else { (*v2, *f2) } })
                .collect()
        )
    }

    pub fn iter(&self) -> std::slice::Iter<(u8, f32)> {
        self.0.iter()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Display for Steps {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0.len())
    }
}

use bitvec::prelude::BitVec;
impl From<BitVec> for Steps {
    #[inline]
    fn from(bs: BitVec) -> Steps {
        Steps(
            bs
                .iter()
                .map(|b| { if *b { (255, 440.0) } else { (0, 0.0) }})
                .collect()
        )
    }
}
