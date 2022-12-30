use std::fs;

fn read_script(filename: &str) -> String {
    fs::read_to_string(format!("./scripts/{}.lua", filename)).ok().unwrap()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let semaphore_script_contents = read_script("semaphore");
    let token_bucket_script_contents = read_script("token_bucket");

    let mut file_content = "\
/// This file is generated with a build script.
///
/// Do not make changes to this file. Instead edit the Lua scripts directly.

"
    .to_string();
    file_content += &format!(
        "pub const SEMAPHORE_SCRIPT: &str = \"\\\n{}\";\n",
        semaphore_script_contents
    );
    file_content += &format!(
        "pub const TOKEN_BUCKET_SCRIPT: &str = \"\\\n{}\";\n",
        token_bucket_script_contents
    );

    fs::write("src/generated.rs", file_content).unwrap();
    Ok(())
}
