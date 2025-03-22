use crate::console::{print_status, Status};

pub struct SyncOperation {
    desc: String,
    verbose: bool,
}

impl SyncOperation {
    pub fn start(desc: String, verbose: bool) -> SyncOperation {
        // print!("  {}", desc);
        SyncOperation { desc, verbose }
    }

    pub fn end(&self, status: Status) {
        if self.verbose || !matches!(status, Status::Skipped) {
            print_status(status, &self.desc);
        }
    }
}
