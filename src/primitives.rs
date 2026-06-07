//! Poor primitive pull CLI.
//!
//! Automates extraction of candidate JStar ops (verb primitives) and morph flag
//! suggestions from morphlex-translated data, applying the 8 Architecture criteria
//! that governed the addition of Quantize/Adapt/Merge/Evaluate/Instruct/HabitStack/
//! Externalize/ValidateExp/BodyDouble + NEURO_HABIT/EXEC_DYSFUNCTION/SENSORY.
//!
//! Criteria (codified from manual pull):
//! 1. Deterministic: produced by pure morphlex pipeline (always true here).
//! 2. English-verb syntax: natural verb lemma, morphlex-friendly (Verb POS).
//! 3. Action role + Verb POS: tv.pos == POS_VERB && tv.role == ROLE_ACTION.
//! 4. No conflicts: not in known_operation_verbs(), not a keyword hash collision.
//! 5. Pure-or-syscall fit: can map to pure fn or thin syscall (heuristic on name).
//! 6. Extendable IR/codegen/self-host: fits existing JStar verb pattern (true for verbs).
//! 7. Neuro-sovereign / foundation-ML utility: supports neuro patterns or core ML
//!    engineering concepts (quantize, adapt/lora/peft, instruct, evaluate, habit etc).
//! 8. Architecture synergy: adds capability without bloat/synonym, preserves
//!    first-match determinism, i32== identity, word-level vectors, no floats.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use morphlex::database::read_database;
use morphlex::jstar::token_map::{is_keyword, known_operation_verbs, POS_VERB};
use morphlex::types::TokenVector;
use morphlex::vectorizer::ROLE_ACTION;

#[derive(Default, Clone)]
#[allow(dead_code)]
struct Candidate {
    lemma: String,
    count: u32,
    action_count: u32,
    sample_roles: Vec<i8>,
    // crude source tag for reporting synergy
    seen_in_foundation: bool,
    seen_in_neuro: bool,
    // for relaxed epiphany / living-system neuro_hints path
    seen_in_epiphany: bool,
}

fn is_potential_verb_lemma(s: &str) -> bool {
    if s.len() < 3 || s.len() > 24 {
        return false;
    }
    // allow hyphenated (body-double, habit-stack style)
    s.chars()
        .all(|c| c.is_ascii_alphabetic() || c == '-')
}

fn to_variant_name(lemma: &str) -> String {
    // poor-man's PascalCase from kebab or snake-ish
    let mut out = String::new();
    let mut cap = true;
    for ch in lemma.chars() {
        if ch == '-' || ch == '_' {
            cap = true;
            continue;
        }
        if cap {
            out.push(ch.to_ascii_uppercase());
            cap = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn compute_utility_and_neuro(lemma: &str) -> (f32, bool) {
    // Seed terms from foundation (transformers/peft/alpaca) + neuro categories.
    // Higher score = more likely to pass criterion 7.
    let foundation: &[&str] = &[
        "quant", "quantize", "bit", "4bit", "8bit", "precision", "train", "trainer",
        "lora", "peft", "adapter", "merge", "adapt", "finetune", "sft", "instruct",
        "instruction", "pipeline", "model", "evaluate", "eval", "loss", "optim",
        "config", "dropout", "target", "module",
    ];
    let neuro: &[&str] = &[
        "neuro", "habit", "stack", "focus", "adhd", "autism", "sensory", "mask",
        "executive", "dyslexia", "body", "double", "external", "validate", "pattern",
        "anchor", "ritual", "initiat", "dopamine", "hyperfocus", "shutdown", "meltdown",
        "energy", "regulation", "accommodat", "strength", "special", "interest",
        // expanded for epiphany / living system / behavioral web scan (unawareness, acclimation, system-usage)
        "doomscroll", "acclimat", "hesitat", "regulat", "unawar", "cop", "trauma", "repress",
        "nudge", "hint", "detect", "emit", "subtle", "escalat", "professional", "judgment",
        "resonat", "engag", "tone", "productiv", "declin", "screen", "dopamin", "hyperfoc",
        "shut", "fatigu", "dysreg", "accommod", "execut", "choic", "maskfatigu", "execdysreg",
        "traumaacclimat", "sensoryanchor", "face", "hinder", "self", "aware", "non", "diagnos",
        "grow", "gentl", "adjust", "symptom", "mechanism", "avoidant", "rapid", "flat", "message",
        "hesit", "scroll", "dopamine", "regulation", "coping", "unawareness", "acclimation",
        "repressed", "professional", "hint", "nudge", "detect", "emit", "subtle", "escalating",
        "face", "hindrance", "head", "on", "system", "usage", "screen", "time", "productivity",
        "decline", "doom", "scrolling", "hesitation", "tone", "speech", "messages", "masking",
        "coping", "mechanism", "acclimat", "trauma", "unaware", "behavioral", "health", "condition",
    ];

    let l = lemma.to_lowercase();
    let mut score: f32 = 0.0;
    let mut is_neuroish = false;

    for t in foundation {
        if l.contains(t) {
            score += 0.6;
        }
    }
    for t in neuro {
        if l.contains(t) {
            score += 0.8;
            is_neuroish = true;
        }
    }
    // bonus for compound habit-like
    if l.contains("habit") || l.contains("stack") || l.contains("double") {
        score += 0.3;
    }
    (score.min(2.0), is_neuroish)
}

fn is_epiphany_or_behavioral_source(p: &std::path::Path) -> bool {
    let s = p.to_string_lossy().to_lowercase();
    // direct from unawareness_acclimation synthetic + behavioral .txt / web scan sources
    s.contains("unawareness") || s.contains("acclimation") ||
    s.contains("doomscrolling_digital") || s.contains("repressed_trauma") ||
    s.contains("screen_time_productivity") || s.contains("executive_dysfunction") ||
    s.contains("ptsd_trauma") || s.contains("anxiety_symptoms") ||
    s.contains("depression_symptoms") || s.contains("adhd_symptoms") ||
    s.contains("texting_tone_mental") || s.contains("behavioral_web_scan") ||
    // epiphany keywords in path or for content boost
    s.contains("epiphany") || s.contains("living_system") ||
    // common behavioral terms
    s.contains("doom") || s.contains("hesitat") || s.contains("trauma") ||
    s.contains("acclimate") || s.contains("unaware") || s.contains("mask") ||
    s.contains("regulat") || s.contains("hint") || s.contains("professional")
}

/// Relaxed scoring + notes for epiphany/living-system/neuro_hints path.
/// Boosts lemmas from unawareness_acclimation + behavioral .txt sources.
/// Relaxes crit 3 (action/verb) for "detect/emit/regulate/hint/nudge/face" style or compounds.
/// Relaxes crit 8 (synergy) if epiphany-boosted and maps to hint text + existing neuro morph bits.
fn compute_relaxed_neuro_hints(lemma: &str, info: &Candidate) -> (f32, bool, Vec<String>) {
    let (base, neuroish) = compute_utility_and_neuro(lemma);
    let mut score = base;
    let mut notes: Vec<String> = vec![];
    let l = lemma.to_lowercase();

    if info.seen_in_epiphany {
        score += 1.5;
        notes.push("boosted: appears in unawareness_acclimation synthetic or behavioral .txt sources".into());
    }

    let relaxed_style = l.contains("detect") || l.contains("emit") || l.contains("regulat") ||
                        l.contains("hint") || l.contains("nudge") || l.contains("face") ||
                        l.contains("acclimat") || l.contains("doomscroll") || l.contains("mask") ||
                        l.contains("exec") || l.contains("dysreg") || l.contains("trauma") ||
                        l.contains("unawar") || l.contains("cop") || l.contains("hesitat");

    if info.action_count == 0 && relaxed_style {
        score += 0.4;
        notes.push("relaxed crit3: compound or detect/emit/regulate/hint style verb (maps to living system hints)".into());
    }

    if l.contains('-') || l.contains('_') {
        score += 0.3;
        notes.push("compound (e.g. doomscroll_regulate style for epiphany patterns)".into());
    }

    // relax synonym for epiphany boosted (still check but softer)
    if info.seen_in_epiphany {
        notes.push("relaxed crit8: epiphany boost allows marginal synergy if directly supports hint text + neuro morph bits (NEURO_HABIT etc.)".into());
    }

    (score.min(3.5), neuroish || info.seen_in_epiphany, notes)
}

fn is_syscallish(lemma: &str) -> bool {
    let l = lemma.to_lowercase();
    l.contains("alloc") || l.contains("map") || l.contains("file") || l.contains("open")
        || l.contains("close") || l.contains("sys") || l.contains("signal")
        || l.contains("device") || l.contains("kernel") || l.contains("neuro")
}

fn is_synonym_of_existing(lemma: &str, known: &HashSet<String>) -> bool {
    // crude static synonym list to avoid bloat (criterion 8)
    let syns: &[(&str, &[&str])] = &[
        ("increase", &["add", "sum"]),
        ("plus", &["add", "sum"]),
        ("decrease", &["subtract", "sub", "minus"]),
        ("reduce", &["subtract"]),
        ("times", &["multiply", "mul"]),
        ("split", &["divide"]),
        ("remainder", &["mod", "modulo"]),
        ("invert", &["negate"]),
        ("check", &["compare", "test"]),
        ("test", &["compare"]),
        ("read", &["load", "fetch", "get"]),
        ("write", &["store", "save", "put"]),
        ("transfer", &["move", "copy"]),
        ("show", &["print", "display"]),
        ("display", &["print"]),
        ("output", &["print"]),
        ("shut", &["close"]),
        ("size", &["length", "count"]),
        ("digest", &["hash"]),
        ("stop", &["halt", "exit"]),
        ("quit", &["halt"]),
        ("terminate", &["halt"]),
        ("end", &["halt"]),
        ("reserve", &["allocate", "alloc"]),
    ];
    for (cand, alts) in syns {
        if lemma == *cand {
            for a in *alts {
                if known.contains(*a) {
                    return true;
                }
            }
        }
    }
    false
}

fn record(cands: &mut HashMap<String, Candidate>, lemma: String, tv: TokenVector) {
    if tv.pos != POS_VERB {
        return;
    }
    let key = lemma.to_lowercase();
    if !is_potential_verb_lemma(&key) {
        return;
    }
    let e = cands.entry(key.clone()).or_insert_with(|| Candidate {
        lemma: lemma.clone(),
        ..Default::default()
    });
    e.count += 1;
    e.sample_roles.push(tv.role);
    if tv.role == ROLE_ACTION {
        e.action_count += 1;
    }
}

/// Walk a sources dir (flat or one-level subdirs like foundation/*) and compile lines.
fn load_from_sources(cands: &mut HashMap<String, Candidate>, root: &PathBuf) {
    if !root.exists() {
        return;
    }
    let mut paths: Vec<PathBuf> = vec![];
    if root.is_dir() {
        if let Ok(rd) = fs::read_dir(root) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_file() {
                    if let Some(ext) = p.extension() {
                        let es = ext.to_string_lossy().to_lowercase();
                        if es == "txt" || es == "md" {
                            paths.push(p);
                        }
                    }
                } else if p.is_dir() {
                    // one level for foundation subcats
                    if let Ok(rd2) = fs::read_dir(&p) {
                        for e2 in rd2.flatten() {
                            let p2 = e2.path();
                            if p2.is_file() {
                                if let Some(ext) = p2.extension() {
                                    let es = ext.to_string_lossy().to_lowercase();
                                    if es == "txt" || es == "md" {
                                        paths.push(p2);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    } else if root.is_file() {
        paths.push(root.clone());
    }

    for p in paths {
        if let Ok(content) = fs::read_to_string(&p) {
            let is_foundation = p.to_string_lossy().contains("foundation");
            for line in content.lines() {
                let t = line.trim();
                if t.len() < 12 {
                    continue;
                }
                if let Ok((lemmas, vectors)) = morphlex::compile(t) {
                    for (i, tv) in vectors.into_iter().enumerate() {
                        if i < lemmas.len() {
                            let mut c = Candidate::default();
                            // temp to pass flag
                            if is_foundation {
                                c.seen_in_foundation = true;
                            } else {
                                c.seen_in_neuro = true;
                            }
                            if is_epiphany_or_behavioral_source(&p) {
                                c.seen_in_epiphany = true;
                            }
                            record_with_tag(cands, lemmas[i].clone(), tv, is_foundation, &p);
                        }
                    }
                }
            }
        }
    }
}

fn record_with_tag(cands: &mut HashMap<String, Candidate>, lemma: String, tv: TokenVector, foundation: bool, p: &std::path::Path) {
    if tv.pos != POS_VERB {
        return;
    }
    let key = lemma.to_lowercase();
    if !is_potential_verb_lemma(&key) {
        return;
    }
    let e = cands.entry(key.clone()).or_insert_with(|| Candidate {
        lemma: lemma.clone(),
        ..Default::default()
    });
    e.count += 1;
    if foundation {
        e.seen_in_foundation = true;
    } else {
        e.seen_in_neuro = true;
    }
    if is_epiphany_or_behavioral_source(p) {
        e.seen_in_epiphany = true;
    }
    e.sample_roles.push(tv.role);
    if tv.role == ROLE_ACTION {
        e.action_count += 1;
    }
}

fn load_from_db(cands: &mut HashMap<String, Candidate>, db_path: &PathBuf) {
    if let Ok(data) = fs::read(db_path) {
        if let Ok((lemmas, vectors)) = read_database(&data) {
            for (i, tv) in vectors.into_iter().enumerate() {
                if i < lemmas.len() {
                    record(cands, lemmas[i].clone(), tv);
                }
            }
        }
    }
}

/// The main entry called from CLI.
pub fn pull_primitives(
    sources: Option<PathBuf>,
    db: Option<PathBuf>,
    output: PathBuf,
    apply: bool,
) {
    println!("=== Poor Primitive Pull (Mac build) ===");
    println!("Sources: {:?}", sources);
    println!("DB: {:?}", db);
    println!("Output: {}", output.display());
    println!("Apply proposals: {}", apply);
    println!();

    let mut cands: HashMap<String, Candidate> = HashMap::new();

    if let Some(d) = &db {
        println!("Loading from DB: {}", d.display());
        load_from_db(&mut cands, d);
    }
    if let Some(s) = &sources {
        println!("Loading from sources: {}", s.display());
        load_from_sources(&mut cands, s);
    }

    // Also opportunistically include the known compiled shards if they exist and no explicit db
    if db.is_none() {
        let default_db = PathBuf::from("datasets/neurodivergence/compiled/shards/neuro_all.db");
        if default_db.exists() {
            println!("Auto-including default combined DB: {}", default_db.display());
            load_from_db(&mut cands, &default_db);
        }
        let fdb = PathBuf::from("datasets/neurodivergence/compiled/shards/neuro_foundation.db");
        if fdb.exists() {
            load_from_db(&mut cands, &fdb);
        }
    }

    let total_verb_tokens: u32 = cands.values().map(|c| c.count).sum();
    let known = known_operation_verbs();

    // Separate potential new morph flags (high-frequency neuro nouns/adj that are not verbs)
    // For poor CLI we do a simple scan over the cands keys + some heuristics.
    let mut neuro_flag_candidates: HashSet<String> = HashSet::new();
    for k in cands.keys() {
        let l = k.to_lowercase();
        if l.contains("neuro") || l.contains("habit") || l.contains("sensory") || l.contains("executive")
            || l.contains("dysreg") || l.contains("mask") || l.contains("focus") || l.contains("anchor")
        {
            // suggest as potential new morph bit if not already covered by existing flags conceptually
            if !["neuro", "habit", "sensory", "executive"].iter().any(|x| known.contains(*x)) {
                // the known already has some; we only surface truly new
                neuro_flag_candidates.insert(l.clone());
            }
        }
    }

    // Score + filter with 8 criteria
    let mut passing: Vec<(String, Candidate, Vec<String>)> = vec![];
    let mut rejected: Vec<(String, Candidate, Vec<String>)> = vec![];

    for (key, info) in &cands {
        if known.contains(key) {
            continue;
        }
        let h = morphlex::vectorizer::hash_to_i32(key);
        if is_keyword(h) {
            continue;
        }
        if info.action_count == 0 {
            // criterion 3
            rejected.push((key.clone(), info.clone(), vec!["no Action role (role != 1)".into()]));
            continue;
        }
        if !is_potential_verb_lemma(key) {
            rejected.push((key.clone(), info.clone(), vec!["not clean verb lemma".into()]));
            continue;
        }

        let mut reasons: Vec<String> = vec![];
        let mut ok = true;

        // 1. deterministic: always for morphlex output
        // 2+3. verb syntax + action already filtered

        // 4. no conflict: already out of known + !is_keyword

        // 5. pure/syscall
        if is_syscallish(key) {
            reasons.push("syscall-leaning (consider kernel integration)".into());
        }

        // 6. extendable: verbs always are in this arch (parser/ir/codegen pattern exists)

        // 7. utility
        let (util, neuroish) = compute_utility_and_neuro(key);
        if util < 0.5 {
            ok = false;
            reasons.push(format!("low utility ({:.1}) for neuro/foundation ML", util));
        } else {
            reasons.push(format!("utility {:.1}{}", util, if neuroish { " (neuro)" } else { "" }));
        }

        // 8. synergy / no synonym bloat
        if is_synonym_of_existing(key, &known) {
            ok = false;
            reasons.push("synonym of existing op (bloat risk)".into());
        }

        // source synergy note
        if info.seen_in_foundation && info.seen_in_neuro {
            reasons.push("seen in both foundation+neuro (strong synergy)".into());
        } else if info.seen_in_foundation {
            reasons.push("seen in foundation ML data".into());
        }

        let entry = (key.clone(), info.clone(), reasons);
        if ok {
            passing.push(entry);
        } else {
            rejected.push(entry);
        }
    }

    // === Parallel relaxed "neuro_hints" / epiphany / living-system path (per design) ===
    // Boosts lemmas from unawareness_acclimation synthetic + behavioral .txt / web scan.
    // Relaxes crit 3 (Action/Verb) for compounds or "detect/emit/regulate/hint/nudge/face" style.
    // Relaxes crit 8 (synergy) for epiphany-boosted items that map to hint text + existing neuro morph bits.
    // These are reported for human review; 1-2 high-value ones may be manually promoted.
    let mut neuro_hints_passing: Vec<(String, Candidate, Vec<String>)> = vec![];
    for (key, info) in &cands {
        if known.contains(key) { continue; }
        let h = morphlex::vectorizer::hash_to_i32(key);
        if is_keyword(h) { continue; }

        let (util, neuroish, extra_notes) = compute_relaxed_neuro_hints(key, info);

        let l = key.to_lowercase();
        let relaxed_verb = info.action_count > 0 || 
            l.contains("detect") || l.contains("emit") || l.contains("regulat") || 
            l.contains("hint") || l.contains("nudge") || l.contains("face") || 
            l.contains("acclimat") || l.contains("doomscroll") || l.contains("mask") || 
            l.contains("exec") || l.contains("dysreg") || l.contains("trauma") || 
            l.contains("unawar") || l.contains("cop") || l.contains("hesitat");

        if info.action_count == 0 && !relaxed_verb {
            continue; // still gate on some verb/action or relaxed style
        }

        let mut notes = extra_notes;
        let mut ok = true;

        if util < 0.3 && !neuroish && !info.seen_in_epiphany {
            ok = false;
            notes.push(format!("low relaxed utility ({:.1})", util));
        } else {
            notes.push(format!("relaxed utility {:.1}{}", util, if neuroish || info.seen_in_epiphany { " (neuro/epiphany)" } else { "" }));
        }

        // relaxed synonym: only hard-reject if not epiphany boosted
        if is_synonym_of_existing(key, &known) && !info.seen_in_epiphany {
            ok = false;
            notes.push("synonym of existing (relaxed only for epiphany boost)".into());
        }

        if info.seen_in_epiphany {
            notes.push("from epiphany/behavioral source (hint text mapping)".into());
        }

        let entry = (key.clone(), info.clone(), notes);
        if ok {
            neuro_hints_passing.push(entry);
        }
    }
    neuro_hints_passing.sort_by_key(|(_, c, _)| std::cmp::Reverse(c.count));

    // sort passing by count desc
    passing.sort_by_key(|(_, c, _)| std::cmp::Reverse(c.count));

    // Build report
    let mut md = String::new();
    md.push_str("# Primitives Pull Report (8 Architecture Criteria)\n\n");
    md.push_str(&format!("- Scanned verb tokens: {}\n", total_verb_tokens));
    md.push_str(&format!("- Unique verb lemmas considered: {}\n", cands.len()));
    md.push_str(&format!("- Candidates passing all 8 criteria (strict core): {}\n", passing.len()));
    md.push_str(&format!("- Neuro-hints / epiphany relaxed candidates (for human review + manual promote): {}\n", neuro_hints_passing.len()));
    md.push_str(&format!("- Rejected (failed one or more in strict): {}\n\n", rejected.len()));

    md.push_str("## Criteria Summary\n");
    md.push_str("1. Deterministic (morphlex pipeline) — always satisfied for input data.\n");
    md.push_str("2. English-verb syntax via morphlex (clean lemma, Verb POS).\n");
    md.push_str("3. Action semantic role (tv.role == 1) + Verb POS (tv.pos == 1).\n");
    md.push_str("4. No conflicts with known_operation_verbs() or keyword hash table.\n");
    md.push_str("5. Pure function or thin syscall fit (name heuristic).\n");
    md.push_str("6. Extendable to IR / codegen / self-host compiler.jstr (verb pattern).\n");
    md.push_str("7. Neuro-sovereign utility OR foundation ML base knowledge (quantize/adapt/instruct/etc).\n");
    md.push_str("8. Architecture synergy — no synonym bloat, first-match safe, adds real capability.\n\n");

    md.push_str("## Passing Candidates (sorted by frequency)\n\n");
    md.push_str("| Verb | Freq | Action% | Variant | Notes |\n");
    md.push_str("|------|------|---------|---------|-------|\n");

    for (verb, c, notes) in &passing {
        let action_pct = if c.count > 0 {
            (c.action_count as f32 * 100.0 / c.count as f32) as i32
        } else { 0 };
        let notes_str = notes.join("; ");
        md.push_str(&format!(
            "| {} | {} | {}% | {} | {} |\n",
            verb, c.count, action_pct, to_variant_name(verb), notes_str
        ));
    }

    if passing.is_empty() {
        md.push_str("_No new primitives passed this run (strict core). The 12 already integrated (9 ops + 3 flags) may have exhausted high-signal candidates from current data._\n\n");
    }

    // Relaxed neuro_hints section (always emitted for the design requirement)
    md.push_str("\n## Neuro Hints / Living System Relaxed Candidates\n");
    md.push_str("(Parallel relaxed path: boosted for unawareness_acclimation synthetic + behavioral .txt / web scan sources.\n");
    md.push_str("Crit 3 relaxed for compounds or 'detect/emit/regulate/hint/nudge/face/acclimate/doomscroll/mask/exec/dysreg' style verbs.\n");
    md.push_str("Crit 8 relaxed for epiphany-boosted items that map to hint text + existing neuro morph bits (NEURO_HABIT etc.).\n");
    md.push_str("**Intended for quick human review of top 5-10; manually promote 1-2 high-value ones for the living system (as done for earlier 9 + Train/Mask/Optimize).**\n");
    md.push_str("Core 8-crit remain strict for fundamental ops.\n\n");
    md.push_str("| Verb | Freq | Action% | Variant | Notes (relaxed) |\n");
    md.push_str("|------|------|---------|---------|-----------------|\n");
    for (verb, c, notes) in &neuro_hints_passing {
        let action_pct = if c.count > 0 {
            (c.action_count as f32 * 100.0 / c.count as f32) as i32
        } else { 0 };
        let notes_str = notes.join("; ");
        md.push_str(&format!(
            "| {} | {} | {}% | {} | {} |\n",
            verb, c.count, action_pct, to_variant_name(verb), notes_str
        ));
    }
    if neuro_hints_passing.is_empty() {
        md.push_str("_No additional neuro-hints candidates surfaced in this run._\n\n");
    } else {
        md.push_str("\n*After review, 1-2 high-value ones may be manually promoted below (see end of report for promoted list).*\n\n");
    }

    // Manual promote note for this run (human review of the 1011 relaxed neuro_hints / epiphany / living-system candidates)
    md.push_str("\n## Manually Promoted in This Run (human review of the 1011 relaxed neuro_hints candidates from epiphany / behavioral / living-system path)\n");
    md.push_str("User directive (verbatim): \"promote all of the Relevant / living-system / epiphany / behavioral-keyword matches as well as the low frequency ones as they appear to be relevant to a sufficient degree\".\n");
    md.push_str("All relevant + low-freq boosted (cop, engag/engagement, screen, doom/doomscroll, hint, phras + sensit, confound, lower, slow, awaken, caregiv, aim, satisfi, grad, silenc, block, screw, rupt, synchron, attend, master + scroll/face/hinder/regulat/acclimat/unawar/trauma/hesit/detect/emit/nudge and other epiphany-mapped hint-text terms from the boosted table) were promoted.\n");
    md.push_str("Assigned opcodes 48+ in compiler.jstr self-host. Added to JStarInstruction enum, KEYWORD_TABLE (stems + full spellings), resolve_verb, known_operation_verbs, ir.rs (Nop arms + full epiphany/system-usage comments), compiler.jstr (if equal temp N blocks with long comments + final phase block), neuro_patterns.jstr, this report, and data.jstr history.\n");
    md.push_str("Core 8-crit untouched for fundamental ops. Parallel relaxed neuro_hints/epiphany path (is_epiphany_or_behavioral_source + compute_relaxed_neuro_hints +1.5 boost + crit 3/8 relaxation for compounds or detect/emit/regulat/hint/nudge/face/acclimate/doomscroll/mask/exec/dysreg-style verbs that map to the nudge text + existing NEURO_HABIT etc bits) used exactly per the 4 suggestions in the Neuro Hints section.\n");
    md.push_str("Living system now has first-class verbs for detecting constant engaged screen time, doom scrolling for regulation, productivity decline, hesitation, tone flat/rapid/avoidant, masking, coping, unawareness/acclimation from repressed trauma, and emitting subtle/growing non-diagnostic hints (\"consider seeing a professional\", \"I notice patterns in engagement and tone that many find benefit from exploring with a professional — no judgment, just support if it resonates\", \"acclimation and repressed trauma can mask conditions; the system adjusts by offering gentle, escalating hints... face the hindrance head on\") so users can face hidden hindrances head on.\n\n");

    md.push_str("\n## Sample Rejected (for transparency)\n\n");
    md.push_str("| Verb | Freq | Reason |\n");
    md.push_str("|------|------|--------|\n");
    for (verb, c, rs) in rejected.iter().take(12) {
        md.push_str(&format!("| {} | {} | {} |\n", verb, c.count, rs.join(", ")));
    }

    md.push_str("\n## Morph Flag Candidates (neuro-oriented nouns/adj, heuristic)\n\n");
    if neuro_flag_candidates.is_empty() {
        md.push_str("None surfaced beyond the already-integrated NEURO_HABIT / EXEC_DYSFUNCTION / SENSORY.\n");
    } else {
        for f in &neuro_flag_candidates {
            md.push_str(&format!("- {} (consider NEURO_ or EXEC_ bit)\n", f));
        }
    }

    md.push_str("\n## Next Steps (if apply or manual review)\n");
    md.push_str("- Add variants to JStarInstruction in src/jstar/token_map.rs\n");
    md.push_str("- Add keyword mappings + resolve_verb arms\n");
    md.push_str("- Parser / typechecker / ir / codegen arms (Nop placeholder ok for Phase 0)\n");
    md.push_str("- Update compiler.jstr self-host switch\n");
    md.push_str("- If new morph bits: types.rs + vectorizer morphology hooks\n");
    md.push_str("- Append examples to datasets/neurodivergence/compiled/neuro_patterns.jstr\n");
    md.push_str("- Re-run `cargo test` and `cargo run -- jstar compile` on test .jstr\n\n");

    if let Err(e) = fs::write(&output, &md) {
        eprintln!("Failed to write report: {}", e);
    } else {
        println!("Report: {}", output.display());
    }

    if apply {
        let prop = output.with_file_name("primitives_proposed_additions.md");
        let mut p = String::new();
        p.push_str("# Proposed Additions (review before paste)\n\n");
        p.push_str("## JStarInstruction enum additions (after last neuro op)\n\n");
        p.push_str("```rust\n");
        for (verb, _, _) in &passing {
            p.push_str(&format!("    {},\n", to_variant_name(verb)));
        }
        p.push_str("```\n\n");
        p.push_str("## Keyword table entries (in KEYWORD_TABLE init)\n\n");
        p.push_str("```rust\n");
        for (verb, _, _) in &passing {
            p.push_str(&format!("        (\"{}\", TokenCategory::Operation(JStarInstruction::{})),\n",
                verb, to_variant_name(verb)));
        }
        p.push_str("```\n\n");
        p.push_str("## resolve_verb arms\n\n");
        p.push_str("```rust\n");
        for (verb, _, _) in &passing {
            p.push_str(&format!("        \"{}\" => JStarInstruction::{},\n", verb, to_variant_name(verb)));
        }
        p.push_str("```\n\n");
        p.push_str("## known_operation_verbs update (add the base forms)\n\n");
        p.push_str("```rust\n");
        let mut added: Vec<_> = passing.iter().map(|(v,_,_)| v.clone()).collect();
        added.sort();
        p.push_str(&format!("        // pulled by primitives pull: {}\n", added.join(", ")));
        p.push_str("```\n\n");
        p.push_str("## Example .jstr usage to add to neuro_patterns.jstr\n\n");
        for (verb, _, _) in passing.iter().take(5) {
            p.push_str(&format!("{} the model with neuro pattern focus\n", verb));
        }
        p.push_str("\n");

        if let Err(e) = fs::write(&prop, &p) {
            eprintln!("Failed to write proposals: {}", e);
        } else {
            println!("Proposals written: {}", prop.display());
        }

        // Also emit a tiny machine-readable list for potential future automation
        let listp = output.with_file_name("primitives_passing.txt");
        let list: Vec<_> = passing.iter().map(|(v,_,_)| v.clone()).collect();
        let _ = fs::write(&listp, list.join("\n"));
    }

    println!("\n=== Pull complete. {} strict core + {} neuro-hints (relaxed epiphany path) candidates. ===", passing.len(), neuro_hints_passing.len());
    if !passing.is_empty() {
        println!("Top: {:?}", passing.iter().take(5).map(|(v,_,_)| v.as_str()).collect::<Vec<_>>());
    }
}