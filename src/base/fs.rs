use std::{fs, io, path::Path};

/// 创建文件路径的所有目录
pub fn create_file_dir_all(file_path: impl AsRef<Path>) -> io::Result<()> {
    if let Some(parent) = file_path.as_ref().parent() {
        fs::create_dir_all(parent)
    } else {
        Ok(())
    }
}
