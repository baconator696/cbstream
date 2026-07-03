use {
    crate::s,
    std::{
        sync::{Arc, OnceLock, RwLock, atomic},
        *,
    },
};
type Res<T> = Result<T, Box<dyn error::Error>>;
static ABORT: OnceLock<Arc<RwLock<bool>>> = OnceLock::new();
pub fn get() -> Res<bool> {
    let a = ABORT.get_or_init(|| init_internal().unwrap());
    return Ok(*a.read().map_err(s!())?);
}
fn init_internal() -> Res<Arc<RwLock<bool>>> {
    let abort = Arc::new(RwLock::new(false));
    let a = abort.clone();
    thread::spawn(move || {
        let term: Arc<atomic::AtomicBool> = Arc::new(atomic::AtomicBool::new(false));
        for sig in signal_hook::consts::TERM_SIGNALS {
            signal_hook::flag::register_conditional_shutdown(*sig, 1, Arc::clone(&term)).unwrap();
            signal_hook::flag::register(*sig, Arc::clone(&term)).unwrap();
        }
        while !term.load(atomic::Ordering::Relaxed) {
            thread::sleep(time::Duration::from_millis(200));
        }
        *a.write().unwrap() = true;
    });
    Ok(abort)
}
