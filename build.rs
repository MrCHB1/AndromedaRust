use std::{
    env, fs,
    path::{Path, PathBuf},
};

const COPY_DIR: &'static str = "assets";
const IGNORE_DIRS: [&'static str; 2] = ["assets/plugins/builtin", "assets/plugins/custom"];

fn copy_dir<P, Q>(from: P, to: Q, ignore_paths: &[PathBuf])
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let to = to.as_ref().to_path_buf();

    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        // Try to canonicalize the path for reliable comparisons.
        // If canonicalize fails, fall back to the original path.
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());

        // If this path (file or directory) is inside any ignored path, skip it.
        if ignore_paths.iter().any(|p| canonical.starts_with(p)) {
            continue;
        }

        let to = to.clone().join(path.file_name().unwrap());

        if path.is_file() {
            fs::copy(&path, &to).unwrap();
        } else if path.is_dir() {
            if !to.exists() {
                fs::create_dir(&to).unwrap();
            }

            copy_dir(&path, to, ignore_paths);
        }
    }
}

fn main() {
    // Build absolute/canonical ignore paths relative to current working directory.
    let cwd = env::current_dir().unwrap();
    let ignore_paths: Vec<PathBuf> = IGNORE_DIRS
        .iter()
        .map(|p| {
            let joined = cwd.join(p);
            joined.canonicalize().unwrap_or(joined)
        })
        .collect();

    let profile = env::var("PROFILE").unwrap();
    let out = PathBuf::from(format!("target/{}/{}", profile, COPY_DIR));

    if out.exists() {
        fs::remove_dir_all(&out).unwrap();
    }

    fs::create_dir_all(&out).unwrap();

    copy_dir(COPY_DIR, &out, &ignore_paths);
}