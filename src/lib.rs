#[macro_export]
macro_rules! breadcumb {
    ($($arg:tt)*) => {
        println!("BREADCUMB: {}", format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! runtime_check {
    ($($arg:tt)*) => {
        println!("RUNTIME CHECK: {}", format_args!($($arg)*));
    };
    () => {
    };
}
