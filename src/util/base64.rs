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

    bytes
        .chunks(3)
        .map(|b| {
            let mut r: u32 = 0x00;
            r += (b[0] as u32) << 0x10;
            r += (b[1] as u32) << 0x08;
            r += b[2] as u32;

            let mut t: [u8; 4] = [0; 4];

            let mut i = 4;
            while i > 0 {
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

pub fn base64_to_bytes(encoded: String) -> Vec<u8> {
    let mut padding = 0;
    let mut res = encoded
        .as_bytes()
        .chunks(4)
        .map(|c| {
            /*
            let mut h = 0u32;
            h <<= c[0];
            h <<= c[1];
            h <<= c[2];
            h <<= c[3];

            let mut u: [u8; 3] = [0u8; 3];
            u[0] = (h >> 24) as u8;
            u[1] = ((h << 8) >> 16) as u8;
            u[2] = ((h << 16) >> 24) as u8;



            u
            */

            let mut h: [u8; 4] = [0u8; 4];
            for e in c.iter().enumerate() {
                if e.1 == &b'=' {
                    h[e.0] = 0;
                    padding += 1;
                    continue;
                }

                // hooo boy
                let mut count = 0;
                for i in INDEX.iter().enumerate() {
                    if i.1 == e.1 {
                        h[e.0] = i.0 as u8;
                        break;
                    }
                    count += 1;
                }

                if count > 0 && h[e.0] == 0 {
                    panic!("could not find e in INDEX: {}", *e.1 as char);
                }
            }

            let mut u = 0u32;
            for e in h {
                u <<= 6;
                u |= e as u32;
            }

            let mut n: [u8; 3] = [0u8; 3];
            n[0] = (u >> 16) as u8;
            n[1] = ((u << 16) >> 24) as u8;
            n[2] = ((u << 24) >> 24) as u8;

            n
        })
        .flatten()
        .collect::<Vec<u8>>();

    if padding > 2 {
        panic!("malformed base64 string")
    }

    while padding > 0 {
        res.pop();
        padding -= 1;
    }

    res
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_STRING_1: &str = "Sally sells sea shells by the sea shore";
    const TEST_STRING_2: &str = "KNTOBTUT, the unification of KNTO and BTUT";
    const TEST_STRING_3: &str = "how did i get here i am not good with computer";

    #[test]
    fn test_base64_encoding_decoding() {
        assert_eq!(
            bytes_to_base64("r".as_bytes().to_vec()),
            bytes_to_base64(base64_to_bytes("cg==".into()))
        );
        assert_eq!(
            bytes_to_base64(TEST_STRING_1.as_bytes().to_vec()),
            bytes_to_base64(base64_to_bytes(
                "U2FsbHkgc2VsbHMgc2VhIHNoZWxscyBieSB0aGUgc2VhIHNob3Jl".into()
            ))
        );
        assert_eq!(
            bytes_to_base64(TEST_STRING_2.as_bytes().to_vec()),
            bytes_to_base64(base64_to_bytes(
                "S05UT0JUVVQsIHRoZSB1bmlmaWNhdGlvbiBvZiBLTlRPIGFuZCBCVFVU".into()
            ))
        );
        assert_eq!(
            bytes_to_base64(TEST_STRING_3.as_bytes().to_vec()),
            bytes_to_base64(base64_to_bytes(
                "aG93IGRpZCBpIGdldCBoZXJlIGkgYW0gbm90IGdvb2Qgd2l0aCBjb21wdXRlcg==".into()
            ))
        );
    }
}
