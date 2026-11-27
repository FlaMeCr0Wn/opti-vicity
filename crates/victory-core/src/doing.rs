//! What a thread is doing, for a crash report: a driver that ends the
//! process inside one of its calls (AMD's shader compiler making a pipeline,
//! `amdxc64.dll`) leaves only its own addresses on the stack, which do not
//! say what it was given. The thread notes it first ([`Doing::new`]); the
//! report prints the note of the thread that stopped ([`now`]).

use std::cell::RefCell;

thread_local! {
    static NOW: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The note of the calling thread, while it lives; the one before comes
/// back when it is dropped.
pub struct Doing(Option<String>);

impl Doing {
    pub fn new(what: String) -> Doing {
        Doing(NOW.try_with(|n| n.try_borrow_mut().ok().map(|mut n| n.replace(what))).ok().flatten().flatten())
    }
}

impl Drop for Doing {
    fn drop(&mut self) {
        let before = self.0.take();
        let _ = NOW.try_with(|n| n.try_borrow_mut().map(|mut n| *n = before));
    }
}

/// What the calling thread noted it is doing, if anything.
pub fn now() -> Option<String> {
    NOW.try_with(|n| n.try_borrow().ok().and_then(|n| n.clone())).ok().flatten()
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_note_lives_as_long_as_its_guard() {
        assert_eq!(super::now(), None);
        {
            let _a = super::Doing::new("a".into());
            {
                let _b = super::Doing::new("b".into());
                assert_eq!(super::now().as_deref(), Some("b"));
            }
            assert_eq!(super::now().as_deref(), Some("a"));
        }
        assert_eq!(super::now(), None);
    }
}
