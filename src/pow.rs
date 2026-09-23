//! Proof-of-work gate over SHA-256.
//!
//! The peer receives a random ASCII prefix and must answer with any string `s` such
//! that `sha256(prefix || s)` starts with `bits` zero bits. [`Pow::solve`] is the
//! reference client side.

use std::hash::{BuildHasher, RandomState};

use sha2::{Digest, Sha256};

use crate::{
    error::{Error, Result},
    gate::Gate,
    session::Session,
};

#[derive(Clone, Copy, Debug)]
pub struct Pow {
    bits: u32,
}

impl Pow {
    pub fn new(bits: u32) -> Self {
        assert!(
            bits <= 64,
            "a proof of work above 64 bits is a denial of service"
        );
        Self { bits }
    }

    /// Whether `sha256(prefix || answer)` has the required leading zero bits.
    pub fn verify(&self, prefix: &str, answer: &str) -> bool {
        leading_zeros(prefix, answer) >= self.bits
    }

    /// Brute-force an answer. Expected cost is `2^bits` hashes.
    pub fn solve(prefix: &str, bits: u32) -> String {
        (0u64..)
            .map(|n| n.to_string())
            .find(|answer| leading_zeros(prefix, answer) >= bits)
            .expect("the search space is not exhausted")
    }

    /// Sixteen hex characters. The standard library's hasher keys are drawn from
    /// the OS once per process and advanced per instance, which is unpredictable
    /// enough to stop precomputation without pulling in an RNG.
    fn prefix(&self, id: u64) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let value = RandomState::new().hash_one((id, nanos));
        format!("{value:016x}")
    }
}

impl Gate for Pow {
    fn admit(&self, session: &mut Session) -> Result<()> {
        if self.bits == 0 {
            return Ok(());
        }
        let prefix = self.prefix(session.info().id);
        session.say(format!(
            "proof of work: send s such that sha256(\"{prefix}\" + s) starts with {} zero bits",
            self.bits
        ))?;
        let answer = session.ask("pow> ")?;
        if self.verify(&prefix, answer.trim()) {
            Ok(())
        } else {
            Err(Error::Denied("proof of work rejected".into()))
        }
    }
}

fn leading_zeros(prefix: &str, answer: &str) -> u32 {
    let digest = Sha256::new()
        .chain_update(prefix)
        .chain_update(answer)
        .finalize();
    let mut zeros = 0;
    for byte in digest {
        zeros += byte.leading_zeros();
        if byte != 0 {
            break;
        }
    }
    zeros
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solved_answers_verify() {
        let pow = Pow::new(12);
        let answer = Pow::solve("abc", 12);
        assert!(pow.verify("abc", &answer));
        assert!(!Pow::new(64).verify("abc", &answer));
    }
}
