use crate::{h, o, s};
use std::sync::{Arc, OnceLock, RwLock, atomic};
use std::*;
type Result<T> = result::Result<T, Box<dyn error::Error>>;
static ABORT: OnceLock<Arc<RwLock<bool>>> = OnceLock::new();
pub fn get() -> Result<bool> {
    let a: &Arc<RwLock<bool>>;
    if ABORT.get().is_none() {
        a = ABORT.get_or_init(|| init_internal().unwrap());
    } else {
        a = ABORT.get().ok_or_else(o!())?;
    }
    return Ok(*a.read().map_err(s!())?);
}
fn init_internal() -> Result<Arc<RwLock<bool>>> {
    let abort = Arc::new(RwLock::new(false));
    let a = abort.clone();
    let header = thread::spawn(move || {
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
    thread::sleep(time::Duration::from_millis(200));
    if header.is_finished() {
        header.join().map_err(h!())?;
    }
    Ok(abort)
}
