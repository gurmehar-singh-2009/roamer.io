use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use dashmap::DashMap;
use nanorand::{Rng, WyRand};
use sha2::{Digest, Sha256};

pub struct Pow {
    pub challenges: DashMap<String, Instant>,
    pub difficulty: u32,
}

impl Pow {
    pub fn new(difficulty: u32) -> Arc<Self> {
        Arc::new(Self {
            challenges: DashMap::new(),
            difficulty,
        })
    }

    pub fn generate_challenge(&self) -> String {
        let mut rng = WyRand::new();
        let challenge: String = (0..32)
            .map(|_| {
                let idx = rng.generate_range(0..36u8);
                if idx < 10 {
                    (b'0' + idx) as char
                } else {
                    (b'a' + idx - 10) as char
                }
            })
            .collect();

        self.challenges.insert(challenge.clone(), Instant::now());

        challenge
    }

    pub fn verify(&self, challenge: &str, nonce: &str) -> bool {
        let Some(entry) = self.challenges.get(challenge) else {
            return false;
        };

        if entry.elapsed() > Duration::from_secs(300) {
            drop(entry);
            self.challenges.remove(challenge);
            return false;
        }

        let input = format!("{}{}", challenge, nonce);
        let hash = Sha256::digest(input.as_bytes());
        let valid = leading_zero_bits(&hash) >= self.difficulty;

        drop(entry);

        if valid {
            self.challenges.remove(challenge);
        }

        valid
    }

    pub fn sweep(&self) {
        self.challenges.retain(|_, t| t.elapsed() < Duration::from_secs(300));
    }
}

fn leading_zero_bits(hash: &[u8]) -> u32 {
    let mut count = 0;
    for byte in hash {
        count += byte.leading_zeros();
        if *byte != 0 {
            break;
        }
    }
    count
}
