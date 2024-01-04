#[cfg(test)]
mod test {
    use anyhow::Ok;

    use crate::error::TestResult;

    #[test]
    fn it_reparents_moved_files() -> TestResult {
        // todo!()
        Ok(())
    }

    // The valid moves:
    // all of these should Just Work (tm), because it's a simple reparent operation
    // move file from root to sub directory
    // move file from sub directory to root
    // move file from subdirectory to another subdirectory
    // rename file

    // but... even if the content doesn't change, then it should be reparented

    // change title of file without moving
    // - change the title
    // - somehow look up the original title? or find pages that have been orphaned
    // - check orphaned pages to see if they reference the original file
    // change title and move
    // warn about files that are orphaned

    // links are checked
}
