//! morphlex CLI -- compile English into a deterministic vector database.
//! PQC encrypted: ML-KEM-1024 (FIPS 203) + ML-DSA-65 (FIPS 204) + AES-256-GCM.

use clap::{Parser, Subcommand};
use morphlex::vectorizer;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "morphlex")]
#[command(about = "Deterministic natural language tokenizer and vector compiler")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a word list file into a PQC-encrypted vector database
    Compile {
        /// Path to input word list (one word per line)
        #[arg(short, long)]
        input: PathBuf,

        /// Output path for the encrypted database
        #[arg(short, long, default_value = "morphlex.db.enc")]
        output: PathBuf,
    },

    /// Compile the system dictionary (/usr/share/dict/words)
    CompileDict {
        /// Output path for the encrypted database
        #[arg(short, long, default_value = "morphlex.db.enc")]
        output: PathBuf,
    },

    /// Tokenize and vectorize a text string (for testing/inspection)
    Tokenize {
        /// The text to tokenize
        text: String,
    },

    /// Inspect a PQC-encrypted database (requires key bundle directory)
    Inspect {
        /// Path to the encrypted database
        #[arg(short, long)]
        database: PathBuf,

        /// Path to the key bundle directory (contains dk.bin, vk.bin)
        #[arg(short, long)]
        keys: PathBuf,
    },

    /// Compile a JStar source file (.jstr) to a native ELF binary
    Jstar {
        #[command(subcommand)]
        action: JStarAction,
    },

    /// Launch the JStar shell (interactive REPL or script execution)
    Jsh {
        /// Optional .jsh script file to execute (omit for REPL mode)
        script: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum JStarAction {
    /// Compile a .jstr source file to a native binary
    Compile {
        /// Path to the .jstr source file
        #[arg(short, long)]
        input: PathBuf,

        /// Additional source files to include (concatenated before main input)
        #[arg(long)]
        include: Vec<PathBuf>,

        /// Output path for the binary
        #[arg(short, long, default_value = "a.out")]
        output: PathBuf,

        /// Use raw tokenization (bypass NLP pipeline, for self-hosting verification)
        #[arg(long)]
        raw: bool,
    },

    /// Parse a .jstr source file and display the AST (for debugging)
    Parse {
        /// Path to the .jstr source file, or inline text with --text
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Inline JStar text to parse
        #[arg(short, long)]
        text: Option<String>,
    },
}

/// Write a PQC key bundle to a directory.
fn write_key_bundle(bundle: &morphlex::database::PqcKeyBundle, dir: &std::path::Path) {
    std::fs::create_dir_all(dir).expect("Failed to create key directory");

    std::fs::write(dir.join("dk.bin"), &bundle.decapsulation_key)
        .expect("Failed to write decapsulation key");
    std::fs::write(dir.join("sk.bin"), &bundle.signing_key)
        .expect("Failed to write signing key");
    std::fs::write(dir.join("vk.bin"), &bundle.verifying_key)
        .expect("Failed to write verifying key");
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile { input, output } => {
            let content = std::fs::read_to_string(&input).expect("Failed to read input file");
            let words: Vec<String> = content.lines().map(|l| l.trim().to_string()).collect();

            let db_path = output.with_extension("db");
            let bundle = morphlex::compile_lexicon(&words, &db_path, &output)
                .expect("Compilation failed");

            let key_dir = output.with_extension("keys");
            write_key_bundle(&bundle, &key_dir);

            println!("Compiled {} words -> {}", words.len(), output.display());
            println!("PQC keys: {}", key_dir.display());
            println!("Signature: {}", output.with_extension("sig").display());

            let _ = std::fs::remove_file(&db_path);
        }

        Commands::CompileDict { output } => {
            let dict_path = "/usr/share/dict/words";
            let content = std::fs::read_to_string(dict_path)
                .expect("System dictionary not found at /usr/share/dict/words");
            let words: Vec<String> = content.lines().map(|l| l.trim().to_string()).collect();

            println!("Compiling {} words...", words.len());

            let db_path = output.with_extension("db");
            let bundle = morphlex::compile_lexicon(&words, &db_path, &output)
                .expect("Compilation failed");

            let key_dir = output.with_extension("keys");
            write_key_bundle(&bundle, &key_dir);

            println!("Compiled -> {}", output.display());
            println!("PQC keys: {}", key_dir.display());
            println!("Signature: {}", output.with_extension("sig").display());

            let _ = std::fs::remove_file(&db_path);
        }

        Commands::Tokenize { text } => {
            let (lemmas, vectors) = morphlex::compile(&text).expect("Tokenization failed");

            for (lemma, tv) in lemmas.iter().zip(vectors.iter()) {
                let id = tv.id;
                let lemma_id = tv.lemma_id;
                let pos = tv.pos;
                let role = tv.role;
                let morph = tv.morph;
                println!("-----------------------------");
                println!("  lemma:    {}", lemma);
                println!("  id:       {} (0x{:08X})", id, id as u32);
                println!("  lemma_id: {} (0x{:08X})", lemma_id, lemma_id as u32);
                println!("  pos:      {:?}", vectorizer::i8_to_pos(pos));
                println!("  role:     {:?}", vectorizer::i8_to_role(role));
                println!("  morph:    0b{:016b}", morph);
                println!("  bytes:    {:?}", tv.to_bytes());
            }
            println!("-----------------------------");
            println!(
                "{} tokens, {} bytes total",
                vectors.len(),
                vectors.len() * 12
            );
        }

        Commands::Jstar { action } => match action {
            JStarAction::Compile { input, include, output, raw } => {
                let mode = if raw { "raw" } else { "nlp" };
                if include.is_empty() {
                    println!("Compiling {} -> {} ({})", input.display(), output.display(), mode);
                    if raw {
                        morphlex::jstar::compile_file_raw(&input, &output)
                            .expect("JStar compilation failed");
                    } else {
                        morphlex::jstar::compile_file(&input, &output)
                            .expect("JStar compilation failed");
                    }
                } else {
                    let mut sources: Vec<PathBuf> = include;
                    sources.push(input.clone());
                    let paths: Vec<&std::path::Path> = sources.iter().map(|p| p.as_path()).collect();
                    println!("Compiling {} files -> {} ({})", paths.len(), output.display(), mode);
                    morphlex::jstar::compile_multi(&paths, &output)
                        .expect("JStar compilation failed");
                }
                println!("Binary written to {}", output.display());
            }
            JStarAction::Parse { input, text } => {
                let source = match (input, text) {
                    (Some(path), _) => {
                        std::fs::read_to_string(&path).expect("Failed to read source file")
                    }
                    (_, Some(text)) => text,
                    (None, None) => {
                        eprintln!("Error: provide --input <file> or --text <code>");
                        std::process::exit(1);
                    }
                };
                let (lemmas, vectors) =
                    morphlex::jstar::tokenize_jstar(&source).expect("Tokenization failed");
                let program = morphlex::jstar::parser::parse(&lemmas, &vectors)
                    .expect("Parse failed");
                let typed = morphlex::jstar::typechecker::check(&program)
                    .expect("Type check failed");
                println!("{:#?}", typed);
            }
        },

        Commands::Jsh { script } => match script {
            Some(path) => {
                morphlex::jsh::scripting::run_script(&path)
                    .expect("Script execution failed");
            }
            None => {
                morphlex::jsh::repl::run().expect("REPL error");
            }
        },

        Commands::Inspect { database, keys } => {
            let dk_bytes = std::fs::read(keys.join("dk.bin"))
                .expect("Failed to read dk.bin from key directory");

            let vk_path = keys.join("vk.bin");
            let vk_bytes = if vk_path.exists() {
                Some(std::fs::read(&vk_path).expect("Failed to read vk.bin"))
            } else {
                None
            };

            let decrypted = morphlex::database::decrypt(
                &database,
                &dk_bytes,
                vk_bytes.as_deref(),
            )
            .expect("Decryption failed");

            let (lemmas, vectors) = morphlex::database::read_database(&decrypted)
                .expect("Failed to parse database");

            println!("{} vectors in database", vectors.len());
            for (i, (lemma, tv)) in lemmas.iter().zip(vectors.iter()).take(20).enumerate() {
                let id = tv.id;
                let pos = tv.pos;
                let role = tv.role;
                println!(
                    "  [{:>5}] {:>20}  id={:>11}  pos={:?}  role={:?}",
                    i,
                    lemma,
                    id,
                    vectorizer::i8_to_pos(pos),
                    vectorizer::i8_to_role(role),
                );
            }
            if vectors.len() > 20 {
                println!("  ... and {} more", vectors.len() - 20);
            }
        }
    }
}
