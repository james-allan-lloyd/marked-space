use owo_colors::{OwoColorize, Stream::Stdout, Style};

const PADDING: usize = 9;

pub enum Status {
    Updated,
    Skipped,
    Created,
    Error,
    Deleted,
    Archived,
    Unarchived,
}

pub fn print_warning(warning_str: &str) {
    println!(
        "{}: {}",
        "warning".if_supports_color(Stdout, |s| s.bright_yellow()),
        warning_str
    );
}

pub fn print_info(info_str: &str) {
    println!("{}{}", " ".repeat(PADDING), info_str);
}

pub fn print_error(info_str: &str) {
    println!(
        "{}: {}",
        "  error".if_supports_color(Stdout, |s| s.bright_red()),
        info_str
    );
}

pub fn print_status(status: Status, status_str: &str) {
    let (label, style) = match status {
        Status::Updated => ("updated", Style::new().cyan()),
        Status::Skipped => ("skipped", Style::new().dimmed()),
        Status::Created => ("created", Style::new().green()),
        Status::Error => ("  error", Style::new().red()),
        Status::Deleted => ("deleted", Style::new()),
        Status::Archived => ("archived", Style::new().blue()),
        Status::Unarchived => ("unarchived", Style::new().blue()),
    };
    println!(
        "{}: {}",
        label.if_supports_color(Stdout, |s| s.style(style)),
        status_str
    );
}
