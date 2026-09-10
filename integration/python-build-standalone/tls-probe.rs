use std::sync::atomic::{AtomicUsize, Ordering};
static DROPS: AtomicUsize = AtomicUsize::new(0);
struct Value;
impl Drop for Value {
    fn drop(&mut self) {
        DROPS.fetch_add(1, Ordering::SeqCst);
    }
}
thread_local! { static VALUE: Value = const { Value }; }
#[unsafe(no_mangle)]
pub extern "C" fn initialize() {
    VALUE.with(|_| {});
}
#[unsafe(no_mangle)]
pub extern "C" fn drops() -> usize {
    DROPS.load(Ordering::SeqCst)
}
