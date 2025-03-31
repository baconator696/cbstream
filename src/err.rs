use any::Any;
use std::*;
#[macro_export]
/// for mapping errors that contain a known string
macro_rules! s {
    () => {
        |e| {
            let caller = panic::Location::caller();
            format!("{}:({}:{}):{}", caller.file(), caller.line(), caller.column(), e)
        }
    };
}
#[macro_export]
/// for mapping known errors
macro_rules! e {
    () => {
        |e| {
            let caller = panic::Location::caller();
            format!("{}:({}:{}):{:?}", caller.file(), caller.line(), caller.column(), e)
        }
    };
}
#[macro_export]
/// for maping Option to Result
macro_rules! o {
    () => {
        || {
            let caller = panic::Location::caller();
            format!("{}:({}:{})", caller.file(), caller.line(), caller.column())
        }
    };
}
#[macro_export]
/// for thread header error mapping
macro_rules! h {
    () => {
        |e| {
            let caller = panic::Location::caller();
            format!(
                "{}:({}:{}):{}",
                caller.file(),
                caller.line(),
                caller.column(),
                $crate::err::header_cast(e)
            )
        }
    };
}
pub fn header_cast(e: Box<dyn Any + Send>) -> String {
    if let Some(e) = e.downcast_ref::<&str>() {
        e.to_string()
    } else if let Ok(e) = e.downcast::<String>() {
        *e
    } else {
        "unknown panic".into()
    }
}
