const INITIAL: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

#[inline(never)]
fn round(index: usize) -> u32 {
    match index {
        0 => 0x428a_2f98,
        1 => 0x7137_4491,
        2 => 0xb5c0_fbcf,
        3 => 0xe9b5_dba5,
        4 => 0x3956_c25b,
        5 => 0x59f1_11f1,
        6 => 0x923f_82a4,
        7 => 0xab1c_5ed5,
        8 => 0xd807_aa98,
        9 => 0x1283_5b01,
        10 => 0x2431_85be,
        11 => 0x550c_7dc3,
        12 => 0x72be_5d74,
        13 => 0x80de_b1fe,
        14 => 0x9bdc_06a7,
        15 => 0xc19b_f174,
        16 => 0xe49b_69c1,
        17 => 0xefbe_4786,
        18 => 0x0fc1_9dc6,
        19 => 0x240c_a1cc,
        20 => 0x2de9_2c6f,
        21 => 0x4a74_84aa,
        22 => 0x5cb0_a9dc,
        23 => 0x76f9_88da,
        24 => 0x983e_5152,
        25 => 0xa831_c66d,
        26 => 0xb003_27c8,
        27 => 0xbf59_7fc7,
        28 => 0xc6e0_0bf3,
        29 => 0xd5a7_9147,
        30 => 0x06ca_6351,
        31 => 0x1429_2967,
        32 => 0x27b7_0a85,
        33 => 0x2e1b_2138,
        34 => 0x4d2c_6dfc,
        35 => 0x5338_0d13,
        36 => 0x650a_7354,
        37 => 0x766a_0abb,
        38 => 0x81c2_c92e,
        39 => 0x9272_2c85,
        40 => 0xa2bf_e8a1,
        41 => 0xa81a_664b,
        42 => 0xc24b_8b70,
        43 => 0xc76c_51a3,
        44 => 0xd192_e819,
        45 => 0xd699_0624,
        46 => 0xf40e_3585,
        47 => 0x106a_a070,
        48 => 0x19a4_c116,
        49 => 0x1e37_6c08,
        50 => 0x2748_774c,
        51 => 0x34b0_bcb5,
        52 => 0x391c_0cb3,
        53 => 0x4ed8_aa4a,
        54 => 0x5b9c_ca4f,
        55 => 0x682e_6ff3,
        56 => 0x748f_82ee,
        57 => 0x78a5_636f,
        58 => 0x84c8_7814,
        59 => 0x8cc7_0208,
        60 => 0x90be_fffa,
        61 => 0xa450_6ceb,
        62 => 0xbef9_a3f7,
        63 => 0xc671_78f2,
        _ => unreachable!(),
    }
}

pub(crate) fn digest(input: &[u8]) -> [u8; 32] {
    let mut state = INITIAL;
    let (blocks, remainder) = input.as_chunks::<64>();
    for block in blocks {
        compress(&mut state, block);
    }

    let mut final_block = [0_u8; 64];
    final_block[..remainder.len()].copy_from_slice(remainder);
    final_block[remainder.len()] = 0x80;
    if remainder.len() >= 56 {
        compress(&mut state, &final_block);
        final_block.fill(0);
    }
    let bit_len = (input.len() as u64).wrapping_mul(8).to_be_bytes();
    final_block[56..].copy_from_slice(&bit_len);
    compress(&mut state, &final_block);

    let mut output = [0_u8; 32];
    for (word, bytes) in state.iter().zip(output.as_chunks_mut::<4>().0) {
        bytes.copy_from_slice(&word.to_be_bytes());
    }
    output
}

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    // SHA-256 needs only the previous sixteen schedule words. Keeping the
    // rolling window is equivalent to the 64-word expansion and leaves tiny
    // no_std Hosts enough stack to validate an assigned Plan.
    let mut schedule = [0_u32; 16];
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for index in 0..64 {
        let word = if index < 16 {
            let start = index * 4;
            u32::from_be_bytes(block[start..start + 4].try_into().unwrap())
        } else {
            let earlier = schedule[(index + 1) & 15];
            let recent = schedule[(index + 14) & 15];
            let s0 = earlier.rotate_right(7) ^ earlier.rotate_right(18) ^ (earlier >> 3);
            let s1 = recent.rotate_right(17) ^ recent.rotate_right(19) ^ (recent >> 10);
            schedule[index & 15]
                .wrapping_add(s0)
                .wrapping_add(schedule[(index + 9) & 15])
                .wrapping_add(s1)
        };
        schedule[index & 15] = word;
        let upper_e = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let choose = (e & f) ^ (!e & g);
        let first = h
            .wrapping_add(upper_e)
            .wrapping_add(choose)
            .wrapping_add(round(index))
            .wrapping_add(word);
        let upper_a = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let majority = (a & b) ^ (a & c) ^ (b & c);
        let second = upper_a.wrapping_add(majority);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(first);
        d = c;
        c = b;
        b = a;
        a = first.wrapping_add(second);
    }
    for (value, compressed) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *value = value.wrapping_add(compressed);
    }
}

#[cfg(test)]
mod tests {
    use super::digest;

    #[test]
    fn matches_fips_vectors_across_padding_boundaries() {
        assert_eq!(
            digest(b""),
            hex("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
        assert_eq!(
            digest(b"abc"),
            hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(
            digest(&[0x5a; 64]),
            hex("cc7321cce5e4409bd8077d58422e1214969059bbd40b4eeb0de0a642f40f7282")
        );
    }

    fn hex(value: &str) -> [u8; 32] {
        let mut result = [0_u8; 32];
        for (index, byte) in result.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
        }
        result
    }
}
