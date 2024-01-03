#[cfg(test)]
mod test {
    use anyhow::Ok;

    use crate::error::TestResult;

    #[test]
    fn it_works() -> TestResult {
        // todo!()
        Ok(())
    }

    // The valid moves:
    // move file from root to sub directory
    // move file from sub directory to root
    // move file from subdirectory to another subdirectory
    // change title of file without moving
    // change title and move
    // rename file
    // warn about files that are orphaned
}
