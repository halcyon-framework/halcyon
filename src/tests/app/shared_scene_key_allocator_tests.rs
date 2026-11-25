    use super::SharedSceneKeyAllocator;
    use crate::compositor::SceneKey;

    #[test]
    fn clones_share_one_monotonic_sequence() {
        let allocator = SharedSceneKeyAllocator::new(SceneKey::BASE.get() + 1);
        let clone = allocator.clone();

        let first = allocator.allocate_key();
        let second = clone.allocate_key();
        let third = allocator.allocate_key();

        assert_eq!(first, SceneKey::new(SceneKey::BASE.get() + 1));
        assert_eq!(second, SceneKey::new(SceneKey::BASE.get() + 2));
        assert_eq!(third, SceneKey::new(SceneKey::BASE.get() + 3));
    }
