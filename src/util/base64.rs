// based off some code i did for the cryptopals challenge some time ago
// i haven't completed it yet; don't ask for the full repo until i do

const INDEX: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn bytes_to_base64(mut bytes: Vec<u8>) -> String {
    let mut result = String::new();
    let mut padding = 0;

    while bytes.len() % 3 != 0 {
        bytes.push(0x00); // REMEMBER: b'0' is NOT 0x00
        padding += 1;
    }

    bytes.chunks(3)
        .map(|b| {
            let mut r: u32 = 0x00;
            r += (b[0] as u32) << 0x10;
            r += (b[1] as u32) << 0x08;
            r += b[2] as u32;

            let mut t: [u8; 4] = [0; 4];

            let mut i = 4; while i > 0 {
                t[i - 1] = (r & !((r >> 6) << 6)) as u8;
                r >>= 6;
                i -= 1;
            }

            t
        })
        .flatten()
        .for_each(|b| result.push(INDEX[b as usize] as char));

    result.truncate(result.len() - padding);
    while padding > 0 {
        result.push('=');
        padding -= 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_STRING_1: &str = "Sally sells sea shells by the sea shore";
    const TEST_STRING_2: &str = "KNTOBTUT, the unification of KNTO and BTUT";
    const TEST_STRING_3: &str = "how did i get here i am not good with computer";

    #[test]
    fn test_base64_encoding() {
        assert_eq!(bytes_to_base64(TEST_STRING_1.as_bytes().to_vec()), "U2FsbHkgc2VsbHMgc2VhIHNoZWxscyBieSB0aGUgc2VhIHNob3Jl");
        assert_eq!(bytes_to_base64(TEST_STRING_2.as_bytes().to_vec()), "S05UT0JUVVQsIHRoZSB1bmlmaWNhdGlvbiBvZiBLTlRPIGFuZCBCVFVU");
        assert_eq!(bytes_to_base64("r".as_bytes().to_vec()), "cg==");
        assert_eq!(bytes_to_base64(TEST_STRING_3.as_bytes().to_vec()), "aG93IGRpZCBpIGdldCBoZXJlIGkgYW0gbm90IGdvb2Qgd2l0aCBjb21wdXRlcg==");
    }
}
