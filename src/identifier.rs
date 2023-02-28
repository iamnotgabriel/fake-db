use std::sync::Mutex;

pub trait Identifier<V> {
    type Id;
    fn new_id(&self, value: &V) -> Self::Id;

    /// Returns `true` if new_id returns a id based on the `value` input.
    /// Returns `false` if new_id returns a id not related to the `value`input.
    fn is_autogenerated(&self) -> bool;
}

pub struct Sequence {
    last_id: Mutex<u32>,
}

impl Sequence {
    pub fn new() -> Self {
        Self {
            last_id: Mutex::new(0),
        }
    }
}

impl Default for Sequence {
    fn default() -> Self {
        Self::new()
    }
}

/// Generates sequencial
impl<V> Identifier<V> for Sequence {
    type Id = u32;
    fn new_id(&self, _: &V) -> Self::Id {
        let mut last_id = self.last_id.lock().unwrap();
        *last_id += 1;

        *last_id
    }

    fn is_autogenerated(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_sequence() {
        let sequence = Sequence::new();

        assert_eq!(sequence.new_id(&()), 1);
        assert_eq!(sequence.new_id(&()), 2);
        assert_eq!(sequence.new_id(&()), 3);
        assert!(!Identifier::<()>::is_autogenerated(&sequence));
    }
}
