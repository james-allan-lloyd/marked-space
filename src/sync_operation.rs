pub struct SyncOperation {
    desc: String,
    verbose: bool,
}

pub enum SyncOperationResult {
    Updated,
    Skipped,
    Created,
    Error,
    Deleted,
}

impl SyncOperation {
    pub fn start(desc: String, verbose: bool) -> SyncOperation {
        // print!("  {}", desc);
        SyncOperation { desc, verbose }
    }

    pub fn end(&self, result: SyncOperationResult) {
        let result_str = match result {
            SyncOperationResult::Updated => "Updated",
            SyncOperationResult::Skipped => "Skipped",
            SyncOperationResult::Created => "Created",
            SyncOperationResult::Error => "Error",
            SyncOperationResult::Deleted => "Deleted",
        };
        if self.verbose || !matches!(result, SyncOperationResult::Skipped) {
            println!("{}:  {}", self.desc, result_str);
        }
    }
}
