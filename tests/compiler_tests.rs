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
