use crate::util::base64;
use rand::{rngs::StdRng, Rng, SeedableRng};

pub fn create_salt() -> String {
    let mut rng = StdRng::from_entropy();
    let mut salt_raw: [u8; 32] = [0; 32];
    rng.fill(&mut salt_raw[..]);
    base64::bytes_to_base64(salt_raw.to_vec())
}
