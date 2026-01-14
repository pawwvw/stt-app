use std::path::PathBuf;
use std::fs;

fn main() {
    tauri_build::build();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = manifest_dir.join("target");
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let output_dir = target_dir.join(&profile).join("whisher");
    let source_dir = manifest_dir.join("whisher");
    
    if source_dir.exists() {
        println!("cargo:warning=Copying whisher directory to {:?}", output_dir);
        if output_dir.exists() {
            let _ = fs::remove_dir_all(&output_dir);
        }
        copy_dir_recursive(&source_dir, &output_dir).expect("Failed to copy whisher directory");
        
        println!("cargo:warning=Successfully copied whisher to target/{}", profile);
    }
}

fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        
        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    
    Ok(())
}
