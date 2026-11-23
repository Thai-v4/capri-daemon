//! L'empreinte SHA-256 d'un fichier, calculée au fil de sa lecture : celle que le manifeste donne
//! pour chaque fichier d'une version.
//!
//! Écrite ici d'après la norme (FIPS 180-4, *Secure Hash Standard*, août 2015 : constantes de la
//! section 4.2.2, valeur initiale de la section 5.3.3, remplissage de la section 5.1.1, calcul de
//! la section 6.2.2), plutôt que tirée d'un crate : aucune dépendance de plus (rien à télécharger,
//! rien qui compile du C, les trois `clippy` de CLAUDE.md tiennent d'eux-mêmes), et une fonction
//! entièrement déterminée, que ses essais vérifient : les constantes recalculées de leur
//! définition (racines des nombres premiers), les exemples de la norme (FIPS 180-2, annexe B), et
//! des empreintes calculées par `shasum -a 256` sur des longueurs qui encadrent le remplissage.
//! Elle sert à vérifier un fichier, pas à garder un secret : rien n'y est à temps constant.

/// Les 64 constantes : les 32 premiers bits de la partie fractionnaire des racines cubiques des
/// 64 premiers nombres premiers (section 4.2.2).
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// La valeur initiale : les 32 premiers bits de la partie fractionnaire des racines carrées des
/// huit premiers nombres premiers (section 5.3.3).
const H0: [u32; 8] =
    [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];

/// Une empreinte en cours de calcul : on lui donne les octets dans l'ordre, par morceaux de
/// n'importe quelle taille, puis on la termine.
#[derive(Clone)]
pub struct Hasher {
    state: [u32; 8],
    /// Les octets d'un bloc pas encore complet.
    pending: [u8; 64],
    pending_len: usize,
    /// Le nombre d'octets reçus en tout.
    length: u64,
}

impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher {
    pub fn new() -> Self {
        Self { state: H0, pending: [0; 64], pending_len: 0, length: 0 }
    }

    /// Ajoute `bytes` à ce qui a déjà été reçu.
    pub fn update(&mut self, mut bytes: &[u8]) {
        self.length = self.length.wrapping_add(bytes.len() as u64);
        if self.pending_len > 0 {
            let taken = (64 - self.pending_len).min(bytes.len());
            self.pending[self.pending_len..self.pending_len + taken].copy_from_slice(&bytes[..taken]);
            self.pending_len += taken;
            bytes = &bytes[taken..];
            if self.pending_len < 64 {
                return;
            }
            let block = self.pending;
            compress(&mut self.state, &block);
            self.pending_len = 0;
        }
        let (blocks, rest) = bytes.as_chunks::<64>();
        for block in blocks {
            compress(&mut self.state, block);
        }
        self.pending[..rest.len()].copy_from_slice(rest);
        self.pending_len = rest.len();
    }

    /// Termine : le remplissage de la section 5.1.1 (un bit à 1, des zéros, puis la longueur en
    /// bits sur 64 bits), et l'empreinte, en octets.
    pub fn finish(mut self) -> [u8; 32] {
        let bits = self.length.wrapping_mul(8);
        let mut tail = [0u8; 128];
        tail[..self.pending_len].copy_from_slice(&self.pending[..self.pending_len]);
        tail[self.pending_len] = 0x80;
        let blocks = if self.pending_len < 56 { 1 } else { 2 };
        tail[blocks * 64 - 8..blocks * 64].copy_from_slice(&bits.to_be_bytes());
        for block in tail[..blocks * 64].as_chunks::<64>().0 {
            compress(&mut self.state, block);
        }
        let mut digest = [0u8; 32];
        for (out, word) in digest.as_chunks_mut::<4>().0.iter_mut().zip(self.state) {
            *out = word.to_be_bytes();
        }
        digest
    }
}

/// L'empreinte de `bytes`, d'un coup.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(bytes);
    hasher.finish()
}

/// L'empreinte écrite comme dans le manifeste : 64 chiffres hexadécimaux minuscules.
pub fn hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Le calcul d'un bloc de 512 bits (section 6.2.2).
fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (word, bytes) in w.iter_mut().zip(block.as_chunks::<4>().0) {
        *word = u32::from_be_bytes(*bytes);
    }
    for t in 16..64 {
        let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
        let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
        w[t] = w[t - 16].wrapping_add(s0).wrapping_add(w[t - 7]).wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for t in 0..64 {
        let big_s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h.wrapping_add(big_s1).wrapping_add(ch).wrapping_add(K[t]).wrapping_add(w[t]);
        let big_s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = big_s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (word, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *word = word.wrapping_add(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les `count` premiers nombres premiers.
    fn primes(count: usize) -> Vec<u64> {
        let mut found = Vec::new();
        let mut candidate = 2u64;
        while found.len() < count {
            if found.iter().all(|prime| !candidate.is_multiple_of(*prime)) {
                found.push(candidate);
            }
            candidate += 1;
        }
        found
    }

    /// Les constantes sont bien celles que la norme définit : recalculées en nombres entiers, sans
    /// aucune virgule flottante. Pour une racine `r` de `p` et sa constante `k`, la valeur
    /// `x = ⌊r⌋·2³² + k` doit encadrer `r·2³²` : `x ≤ r·2³² < x + 1`, soit, à la puissance `n`
    /// (2 ou 3), `xⁿ ≤ p·2³²ⁿ < (x + 1)ⁿ`.
    #[test]
    fn the_constants_are_those_the_standard_defines() {
        fn brackets(root_of: u64, power: u32, integer: u128, fraction: u32) -> bool {
            let x = (integer << 32) + u128::from(fraction);
            let target = u128::from(root_of) << (32 * power);
            x.pow(power) <= target && target < (x + 1).pow(power)
        }
        for (prime, k) in primes(64).into_iter().zip(K) {
            let integer = (1u128..).take_while(|n| n.pow(3) <= u128::from(prime)).last().expect("≥ 1");
            assert!(brackets(prime, 3, integer, k), "K pour {prime} : {k:#010x}");
        }
        for (prime, h) in primes(8).into_iter().zip(H0) {
            let integer = (1u128..).take_while(|n| n.pow(2) <= u128::from(prime)).last().expect("≥ 1");
            assert!(brackets(prime, 2, integer, h), "H pour {prime} : {h:#010x}");
        }
    }

    /// Les exemples de la norme (FIPS 180-2, annexe B : un bloc, deux blocs, un million de « a »),
    /// et le message vide.
    #[test]
    fn the_standard_s_examples() {
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(
            hex(&sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let million = vec![b'a'; 1_000_000];
        assert_eq!(
            hex(&sha256(&million)),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    /// Des longueurs qui encadrent le remplissage (55 et 56 octets : un bloc de fin ou deux ; 63,
    /// 64, 65 : un bloc tout juste plein), comparées à ce que calcule `shasum -a 256` (macOS,
    /// le 23 novembre 2026) sur les mêmes octets : `(i·7 + 3) mod 256`, pour `i` de 0 à n − 1.
    #[test]
    fn lengths_around_the_padding_agree_with_the_system_s_shasum() {
        let expected = [
            (1, "084fed08b978af4d7d196a7446a86b58009e636b611db16211b65a9aadff29c5"),
            (55, "e7313d333c272e639f790978283f9eb392e843d0f29b7016828bb1daa4aac70b"),
            (56, "4324d65f3c103567f5589c710bc08f8523f929a9272e3af36fc968e52abc6c27"),
            (63, "81c80242132f230c3bd41b3e63bbcff16107339549214a99614ff26664625055"),
            (64, "39e3d7b6b5d075d37d053ad89b24b41bef4f3c29760c84447cab3f3be1882241"),
            (65, "aacca6ff74fdbb296d165a45cecfa04e5127bc008770fbbdd48006f2d2fae95e"),
            (119, "9ce7368e4daf32341631b492e80359dc9f594b48453cd0dd5bf0b19279cc177e"),
            (120, "7836b787757e95e58b3ca5aec90b1b004e8deba1e50e9675af9cabf1a13a04b5"),
            (127, "a8d23e75d936f303d248888d9b165ee543f4cbafcad3c9dd2a79bd84faa11d07"),
            (128, "d2742f1f4ac6bb7ca2b239ee18402ba8b3f9f8e652d2a72973c2b9ba11c08cf6"),
            (1000, "1e9bc38cbf860b9ec31918b065f9b52476c549a782e0e7990bed8ce3868d2371"),
        ];
        for (length, digest) in expected {
            let bytes: Vec<u8> = (0..length).map(|i| ((i * 7 + 3) % 256) as u8).collect();
            assert_eq!(hex(&sha256(&bytes)), digest, "{length} octets");
        }
    }

    /// Par morceaux de n'importe quelle taille, la même empreinte qu'en un seul coup.
    #[test]
    fn the_pieces_do_not_matter() {
        let bytes: Vec<u8> = (0..1000u32).map(|i| ((i * 7 + 3) % 256) as u8).collect();
        let whole = sha256(&bytes);
        for size in [1, 3, 55, 63, 64, 65, 128, 999] {
            let mut hasher = Hasher::new();
            for piece in bytes.chunks(size) {
                hasher.update(piece);
            }
            hasher.update(&[]);
            assert_eq!(hasher.finish(), whole, "morceaux de {size}");
        }
    }
}
