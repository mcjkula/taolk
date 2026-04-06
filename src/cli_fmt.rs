// ANSI terminal formatting for CLI output.

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";
pub const CYAN: &str = "\x1b[96m"; // values, fees, amounts
pub const GREEN: &str = "\x1b[92m"; // success
pub const RED: &str = "\x1b[91m"; // errors
pub const YELLOW: &str = "\x1b[93m"; // labels
pub const WHITE: &str = "\x1b[97m"; // headers, emphasis
pub const MAGENTA: &str = "\x1b[95m"; // addresses

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
