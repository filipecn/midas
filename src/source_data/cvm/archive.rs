use super::CvmError;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedArchive {
    pub files: Vec<PathBuf>,
    pub bytes_written: u64,
}

/// Extract an archive while rejecting path traversal and symlink entries.
/// The source archive is kept intact.
pub fn extract_zip(
    archive_path: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<ExtractedArchive, CvmError> {
    let destination = destination.as_ref();
    fs::create_dir_all(destination)?;
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut files = Vec::new();
    let mut bytes_written = 0;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let Some(relative_path) = entry.enclosed_name() else {
            return Err(CvmError::UnsafeArchivePath(entry.name().to_owned()));
        };
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(CvmError::UnsafeArchivePath(entry.name().to_owned()));
        }
        let output = destination.join(relative_path);
        if entry.is_dir() {
            fs::create_dir_all(&output)?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&output)?;
        bytes_written += io::copy(&mut entry, &mut out)?;
        out.sync_all()?;
        files.push(output);
    }
    Ok(ExtractedArchive {
        files,
        bytes_written,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn extracts_archive_and_keeps_source_file() {
        let unique = format!("midas-cvm-{}", std::process::id());
        let root = std::env::temp_dir().join(unique);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("resource.zip");
        let file = File::create(&archive_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("nested/data.txt", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"cvm fixture").unwrap();
        writer.finish().unwrap();

        let result = extract_zip(&archive_path, root.join("out")).unwrap();
        assert_eq!(result.files.len(), 1);
        assert_eq!(fs::read(&result.files[0]).unwrap(), b"cvm fixture");
        assert!(archive_path.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
