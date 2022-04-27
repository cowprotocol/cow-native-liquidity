use primitive_types::H160;

/// Erc20 token pair specified by two contract addresses.
#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TokenPair(H160, H160);

impl TokenPair {
    /// Create a new token pair from two addresses.
    /// The addresses must not be the equal.
    pub fn new(token_a: H160, token_b: H160) -> Option<Self> {
        match token_a.cmp(&token_b) {
            std::cmp::Ordering::Less => Some(Self(token_a, token_b)),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => Some(Self(token_b, token_a)),
        }
    }

    /// Used to determine if `token` is among the pair.
    pub fn contains(&self, token: &H160) -> bool {
        self.0 == *token || self.1 == *token
    }

    /// Returns the token in the pair which is not the one passed in, or None if token passed in is not part of the pair
    pub fn other(&self, token: &H160) -> Option<H160> {
        if &self.0 == token {
            Some(self.1)
        } else if &self.1 == token {
            Some(self.0)
        } else {
            None
        }
    }

    /// The first address is always the lower one.
    /// The addresses are never equal.
    pub fn get(&self) -> (H160, H160) {
        (self.0, self.1)
    }

    /// Lowest element according to Ord trait.
    pub fn first_ord() -> Self {
        Self(
            H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
        )
    }
}

impl IntoIterator for TokenPair {
    type Item = H160;
    type IntoIter = std::iter::Chain<std::iter::Once<H160>, std::iter::Once<H160>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.0).chain(std::iter::once(self.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    impl Default for TokenPair {
        fn default() -> Self {
            Self::new(H160::from_low_u64_be(0), H160::from_low_u64_be(1)).unwrap()
        }
    }

    impl<'a> IntoIterator for &'a TokenPair {
        type Item = &'a H160;
        type IntoIter = std::iter::Chain<std::iter::Once<&'a H160>, std::iter::Once<&'a H160>>;

        fn into_iter(self) -> Self::IntoIter {
            std::iter::once(&self.0).chain(std::iter::once(&self.1))
        }
    }

    #[test]
    fn token_pair_contains() {
        let token_a = H160::from_low_u64_be(0);
        let token_b = H160::from_low_u64_be(1);
        let token_c = H160::from_low_u64_be(2);
        let pair = TokenPair::new(token_a, token_b).unwrap();

        assert!(pair.contains(&token_a));
        assert!(pair.contains(&token_b));
        assert!(!pair.contains(&token_c));
    }

    #[test]
    fn token_pair_other() {
        let token_a = H160::from_low_u64_be(0);
        let token_b = H160::from_low_u64_be(1);
        let token_c = H160::from_low_u64_be(2);
        let pair = TokenPair::new(token_a, token_b).unwrap();

        assert_eq!(pair.other(&token_a), Some(token_b));
        assert_eq!(pair.other(&token_b), Some(token_a));
        assert_eq!(pair.other(&token_c), None);
    }

    #[test]
    fn token_pair_is_sorted() {
        let token_a = H160::from_low_u64_be(0);
        let token_b = H160::from_low_u64_be(1);
        let pair_0 = TokenPair::new(token_a, token_b).unwrap();
        let pair_1 = TokenPair::new(token_b, token_a).unwrap();
        assert_eq!(pair_0, pair_1);
        assert_eq!(pair_0.get(), pair_1.get());
        assert_eq!(pair_0.get().0, token_a);
    }

    #[test]
    fn token_pair_cannot_be_equal() {
        let token = H160::from_low_u64_be(1);
        assert_eq!(TokenPair::new(token, token), None);
    }

    #[test]
    fn token_pair_iterator() {
        let token_a = H160::from_low_u64_be(0);
        let token_b = H160::from_low_u64_be(1);
        let pair = TokenPair::new(token_a, token_b).unwrap();

        let mut iter = (&pair).into_iter();
        assert_eq!(iter.next(), Some(&token_a));
        assert_eq!(iter.next(), Some(&token_b));
        assert_eq!(iter.next(), None);

        let mut iter = pair.into_iter();
        assert_eq!(iter.next(), Some(token_a));
        assert_eq!(iter.next(), Some(token_b));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn token_pair_ordering() {
        let token_a = H160::from_low_u64_be(0);
        let token_b = H160::from_low_u64_be(1);
        let token_c = H160::from_low_u64_be(2);
        let pair_ab = TokenPair::new(token_a, token_b).unwrap();
        let pair_bc = TokenPair::new(token_b, token_c).unwrap();
        let pair_ca = TokenPair::new(token_c, token_a).unwrap();

        assert_eq!(pair_ab.cmp(&pair_bc), Ordering::Less);
        assert_eq!(pair_ab.cmp(&pair_ca), Ordering::Less);
        assert_eq!(pair_bc.cmp(&pair_ca), Ordering::Greater);
        assert_eq!(pair_ab.cmp(&TokenPair::first_ord()), Ordering::Equal);
    }
}
