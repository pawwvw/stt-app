use std::path::PathBuf;
use std::fs;

fn main() {
    tauri_build::build();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target_dir = manifest_dir.join("target");
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let output_dir = target_dir.join(&profile);
    let source_dir = manifest_dir.join("whisher");
    
    if source_dir.exists() {
        println!("cargo:warning=Copying whisher files to {:?}", output_dir);
        
        let cli_name = if cfg!(target_os = "windows") {
            "whisper-cli.exe"
        } else {
            "whisper-cli"
        };
        
        let cli_src = source_dir.join(cli_name);
        let cli_dst = output_dir.join(cli_name);
        if cli_src.exists() {
            fs::copy(&cli_src, &cli_dst).expect("Failed to copy whisper-cli");
            println!("cargo:warning=Copied {} to {:?}", cli_name, cli_dst);
        }
        
        let models_dir = source_dir.join("models");
        if models_dir.exists() {
            let output_models = output_dir.join("models");
            fs::create_dir_all(&output_models).expect("Failed to create models dir");
            
            for entry in fs::read_dir(&models_dir).expect("Failed to read models dir") {
                let entry = entry.expect("Failed to read entry");
                if entry.file_type().expect("Failed to get file type").is_file() {
                    let dst = output_models.join(entry.file_name());
                    fs::copy(entry.path(), &dst).expect("Failed to copy model");
                    println!("cargo:warning=Copied model {:?}", entry.file_name());
                }
            }
        }
        
        println!("cargo:rerun-if-changed={}", source_dir.display());
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
