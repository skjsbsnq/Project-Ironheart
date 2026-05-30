pub fn log_bool_toggle(name: &str, enabled: bool) {
    println!("[debug] {name} {}", if enabled { "ON" } else { "OFF" });
}

pub fn log_value<T: std::fmt::Display>(name: &str, value: T) {
    println!("[debug] {name} {value}");
}
