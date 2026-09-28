//! Proof-of-work gate over SHA-256.
//!
//! The peer receives a random prefix and must answer with any string `s` such that
//! `sha256(prefix || s)` starts with `bits` zero bits. [`Challenge::solve`] is the
//! client side. The prompt, the challenge text and the prefix source are all replaceable.

use std::{
    borrow::Cow,
    fmt,
    hash::{BuildHasher, RandomState},
};

use sha2::{Digest, Sha256};

use crate::{
    error::{Error, Result},
    gate::Gate,
    session::{Info, Session},
};

type Message = Box<dyn Fn(&Challenge) -> String + Send + Sync>;
type Prefix = Box<dyn Fn(&Info) -> String + Send + Sync>;

/// One puzzle: find `s` with `sha256(prefix || s)` starting with `bits` zero bits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Challenge {
    /// Random text the answer is appended to.
    pub prefix: String,
    /// Leading zero bits the digest needs.
    pub bits: u32,
}

impl Challenge {
    /// Whether `answer` solves this challenge.
    pub fn verify(&self, answer: &str) -> bool {
        Pow::zeros(&self.prefix, answer) >= self.bits
    }

    /// Brute-force an answer. Expected cost is `2^bits` hashes.
    ///
    /// # Panics
    ///
    /// If no decimal number up to `u64::MAX` is an answer.
    pub fn solve(&self) -> String {
        (0..=u64::MAX)
            .map(|n| n.to_string())
            .find(|answer| self.verify(answer))
            .expect("the search space is not exhausted")
    }
}

impl fmt::Display for Challenge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "proof of work: send s such that sha256(\"{}\" + s) starts with {} zero bits",
            self.prefix, self.bits
        )
    }
}

/// A [`Gate`] that makes each peer solve a [`Challenge`] first.
pub struct Pow {
    bits: u32,
    prompt: Cow<'static, str>,
    refusal: Cow<'static, str>,
    message: Message,
    prefix: Prefix,
}

impl Pow {
    /// Require `bits` leading zero bits; `0` lets everyone through.
    ///
    /// # Panics
    ///
    /// If `bits` is above 64.
    pub fn new(bits: u32) -> Self {
        assert!(
            bits <= 64,
            "a proof of work above 64 bits is a denial of service"
        );
        Self {
            bits,
            prompt: "pow> ".into(),
            refusal: "proof of work rejected".into(),
            message: Box::new(Challenge::to_string),
            prefix: Box::new(Self::random),
        }
    }

    /// The difficulty.
    pub fn bits(&self) -> u32 {
        self.bits
    }

    /// Written before reading the answer.
    pub fn prompt(mut self, prompt: impl Into<Cow<'static, str>>) -> Self {
        self.prompt = prompt.into();
        self
    }

    /// What the peer sees on a wrong answer.
    pub fn refusal(mut self, refusal: impl Into<Cow<'static, str>>) -> Self {
        self.refusal = refusal.into();
        self
    }

    /// How the challenge is worded; defaults to its `Display`.
    pub fn message<F>(mut self, message: F) -> Self
    where
        F: Fn(&Challenge) -> String + Send + Sync + 'static,
    {
        self.message = Box::new(message);
        self
    }

    /// Where prefixes come from. Must be unpredictable, or answers can be
    /// precomputed.
    pub fn prefix<F>(mut self, prefix: F) -> Self
    where
        F: Fn(&Info) -> String + Send + Sync + 'static,
    {
        self.prefix = Box::new(prefix);
        self
    }

    /// A fresh challenge for this connection.
    pub fn challenge(&self, info: &Info) -> Challenge {
        Challenge {
            prefix: (self.prefix)(info),
            bits: self.bits,
        }
    }

    fn zeros(prefix: &str, answer: &str) -> u32 {
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

    /// Sixteen hex characters. The standard library's hasher keys come from the OS
    /// once per process and advance per instance, which is unpredictable enough to
    /// stop precomputation without pulling in an RNG.
    fn random(info: &Info) -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        format!("{:016x}", RandomState::new().hash_one((info.id, nanos)))
    }
}

impl Gate for Pow {
    fn admit(&self, session: &mut Session) -> Result<()> {
        if self.bits == 0 {
            return Ok(());
        }
        let challenge = self.challenge(session.info());
        session.say((self.message)(&challenge))?;
        let answer = session.ask(&self.prompt)?;
        if challenge.verify(answer.trim()) {
            Ok(())
        } else {
            Err(Error::denied(self.refusal.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solved_answers_verify() {
        let easy = Challenge {
            prefix: "abc".into(),
            bits: 12,
        };
        let answer = easy.solve();
        assert!(easy.verify(&answer));
        assert!(!Challenge { bits: 64, ..easy }.verify(&answer));
    }
}
