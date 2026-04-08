pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const CYAN: &str = "\x1b[96m";
pub const GREEN: &str = "\x1b[92m";
pub const RED: &str = "\x1b[91m";
pub const YELLOW: &str = "\x1b[93m";
pub const WHITE: &str = "\x1b[97m";
pub const MAGENTA: &str = "\x1b[95m";

pub fn success(msg: &str) {
    eprintln!("{GREEN}{BOLD}\u{2714}{RESET} {GREEN}{msg}{RESET}");
}

pub fn error(msg: &str) {
    eprintln!("{RED}{BOLD}\u{2718}{RESET} {RED}{msg}{RESET}");
}

pub fn label(name: &str, value: &str) {
    eprintln!("  {YELLOW}{name}:{RESET}  {value}");
}

pub fn label_magenta(name: &str, value: &str) {
    eprintln!("  {YELLOW}{name}:{RESET}  {MAGENTA}{value}{RESET}");
}

pub fn label_cyan(name: &str, value: &str) {
    eprintln!("  {YELLOW}{name}:{RESET}  {CYAN}{value}{RESET}");
}

pub fn label_dim(name: &str, value: &str) {
    eprintln!("  {YELLOW}{name}:{RESET}  {DIM}{value}{RESET}");
}

pub fn header(msg: &str) {
    eprintln!("{BOLD}{WHITE}{msg}{RESET}");
}

pub fn hint(msg: &str) {
    eprintln!("{DIM}{msg}{RESET}");
}

pub fn blank() {
    eprintln!();
}
