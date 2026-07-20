use morphlex::jstar;
use morphlex::jstar::codegen::{self, Arch};
use morphlex::types::MorphResult;

fn compile_to_ir(source: &str) -> MorphResult<jstar::ir::IrProgram> {
    let (originals, lemmas, vectors) = jstar::tokenize_jstar(source)?;
    let ast = jstar::parser::parse(&originals, &lemmas, &vectors)?;
    let typed = jstar::typechecker::check(&ast)?;
    Ok(jstar::ir::lower(&typed)?)
}

#[test]
fn test_compile_simple_math_aarch64() {
    let source = include_str!("data/simple_math.jstr");
    let program = compile_to_ir(source).unwrap();
    let code = codegen::generate(Arch::Aarch64, &program).unwrap();
    assert!(!code.text.is_empty(), "AArch64 backend produced no machine code");
}

#[test]
#[ignore = "requires aarch64-linux-gnu-objdump"]
fn test_aarch64_disassembly_has_add_mov_ret() {
    let source = include_str!("data/simple_math.jstr");
    let program = compile_to_ir(source).unwrap();
    let code = codegen::generate(Arch::Aarch64, &program).unwrap();

    let dir = std::env::temp_dir().join("jstar_aarch64_disasm_test");
    std::fs::create_dir_all(&dir).unwrap();
    let bin_path = dir.join("text.bin");
    std::fs::write(&bin_path, &code.text).unwrap();

    let output = std::process::Command::new("aarch64-linux-gnu-objdump")
        .args([
            "-D",
            "-b",
            "binary",
            "-m",
            "aarch64",
            bin_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run aarch64-linux-gnu-objdump");

    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("add"),
        "expected add instruction, got:\n{text}"
    );
    assert!(
        text.contains("mov"),
        "expected mov instruction, got:\n{text}"
    );
    assert!(
        text.contains("ret"),
        "expected ret instruction, got:\n{text}"
    );
}
