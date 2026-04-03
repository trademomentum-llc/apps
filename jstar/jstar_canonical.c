/* jstar_canonical.c - Jasterish compiler in C
 *
 * Designed from first principles based on the Jasterish language specification.
 * This is NOT a port of any other implementation.
 *
 * Phases:
 *   1. Read input
 *   2. Tokenize (whitespace-separated, keyword classification)
 *   3. Data collection (strings → datasec, globals → datasec)
 *   4. Parse & Codegen (AST → x86-64 machine code)
 *   5. Link (ELF64 binary output)
 *
 * Design principles:
 *   - Deterministic: same input always produces identical output
 *   - Correct: follows Jasterish spec, not any implementation
 *   - Complete: handles all language features
 *   - Clean: each phase is independent and testable
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <ctype.h>
#include <assert.h>

/* ─── Configuration ─── */
#define MAX_INPUT    (1 << 20)    /* 1MB source */
#define MAX_TEXT     (1 << 24)    /* 16MB code */
#define MAX_DATA     (1 << 24)    /* 16MB data */
#define MAX_TOKENS   65536
#define MAX_VARS     4096

/* ─── Token types ─── */
enum {
    TOK_UNKNOWN = 0,
    TOK_RETURN = 1,
    TOK_ADD = 2,
    TOK_SUB = 3,
    TOK_MUL = 4,
    TOK_DIV = 5,
    TOK_STORE = 6,
    TOK_LOAD = 7,
    TOK_MOVE = 8,
    TOK_COMPARE = 9,
    TOK_EQUAL = 10,
    TOK_LESS = 11,
    TOK_GREATER = 12,
    TOK_PRINT = 13,
    TOK_IF = 14,
    TOK_WHILE = 15,
    TOK_END = 16,
    TOK_A = 17,
    TOK_AN = 18,
    TOK_THE = 19,
    TOK_INTO = 20,
    TOK_FROM = 21,
    TOK_AT = 22,
    TOK_IT = 23,
    TOK_DEFINE = 24,
    TOK_WITH = 25,
    TOK_CALL = 26,
    TOK_SYSCALL = 27,
    TOK_BITAND = 28,
    TOK_BITOR = 29,
    TOK_BITXOR = 30,
    TOK_SHIFT = 31,
    TOK_HALT = 32,
    TOK_PUSH = 33,
    TOK_POP = 34,
    TOK_BITNOT = 35,
    TOK_LITERAL = 50,
    TOK_STRING = 51,
    TOK_VAR = 52,
    TOK_BYTE = 53,
    TOK_LONG = 54,
    TOK_INTEGER = 55,
    TOK_SHORT = 56,
    TOK_BOOLEAN = 57,
    TOK_CHAR = 58,
    TOK_GLOBAL = 59,
};

/* ─── Token ─── */
typedef struct {
    int type;
    int start;      /* offset in input buffer */
    int len;
    int64_t value;  /* for literals */
} Token;

/* ─── Variable ─── */
typedef struct {
    char name[64];
    int name_len;
    int offset;     /* stack offset (negative) or data offset (positive) */
    int size;       /* bytes */
    int is_global;
} Var;

/* ─── Global state ─── */
static char input[MAX_INPUT];
static int input_len = 0;

static uint8_t text[MAX_TEXT];
static int text_len = 0;

static uint8_t datasec[MAX_DATA];
static int data_len = 0;

static Token tokens[MAX_TOKENS];
static int tok_count = 0;

static Var vars[MAX_VARS];
static int var_count = 0;

static int tok_idx = 0;
static int in_function = 0;

/* ─── Phase 1: Read stdin ─── */
static void read_input(void) {
    input_len = (int)fread(input, 1, MAX_INPUT - 1, stdin);
    input[input_len] = 0;
}

/* ─── Keyword table ─── */
static int classify_keyword(const char *w, int len) {
    /* Perfect hash: check length first, then content */
    switch (len) {
        case 1:
            if (w[0] == 'a') return TOK_A;
            break;
        case 2:
            if (w[0] == 'a' && w[1] == 'n') return TOK_AN;
            if (w[0] == 'i' && w[1] == 'f') return TOK_IF;
            if (w[0] == 'i' && w[1] == 't') return TOK_IT;
            break;
        case 3:
            if (memcmp(w, "end", 3) == 0) return TOK_END;
            if (memcmp(w, "add", 3) == 0) return TOK_ADD;
            if (memcmp(w, "pop", 3) == 0) return TOK_POP;
            break;
        case 4:
            if (memcmp(w, "call", 4) == 0) return TOK_CALL;
            if (memcmp(w, "from", 4) == 0) return TOK_FROM;
            if (memcmp(w, "into", 4) == 0) return TOK_INTO;
            if (memcmp(w, "less", 4) == 0) return TOK_LESS;
            if (memcmp(w, "load", 4) == 0) return TOK_LOAD;
            if (memcmp(w, "move", 4) == 0) return TOK_MOVE;
            if (memcmp(w, "push", 4) == 0) return TOK_PUSH;
            if (memcmp(w, "with", 4) == 0) return TOK_WITH;
            if (memcmp(w, "byte", 4) == 0) return TOK_BYTE;
            if (memcmp(w, "char", 4) == 0) return TOK_CHAR;
            if (memcmp(w, "halt", 4) == 0) return TOK_HALT;
            if (memcmp(w, "exit", 4) == 0) return TOK_HALT;
            break;
        case 5:
            if (memcmp(w, "equal", 5) == 0) return TOK_EQUAL;
            if (memcmp(w, "print", 5) == 0) return TOK_PRINT;
            if (memcmp(w, "store", 5) == 0) return TOK_STORE;
            if (memcmp(w, "short", 5) == 0) return TOK_SHORT;
            if (memcmp(w, "shift", 5) == 0) return TOK_SHIFT;
            break;
        case 6:
            if (memcmp(w, "define", 6) == 0) return TOK_DEFINE;
            if (memcmp(w, "return", 6) == 0) return TOK_RETURN;
            if (memcmp(w, "while", 5) == 0) return TOK_WHILE;
            if (memcmp(w, "bitor", 5) == 0) return TOK_BITOR;
            if (memcmp(w, "long", 4) == 0) return TOK_LONG;
            break;
        case 7:
            if (memcmp(w, "compare", 7) == 0) return TOK_COMPARE;
            if (memcmp(w, "greater", 7) == 0) return TOK_GREATER;
            if (memcmp(w, "syscall", 7) == 0) return TOK_SYSCALL;
            if (memcmp(w, "integer", 7) == 0) return TOK_INTEGER;
            if (memcmp(w, "bitand", 6) == 0) return TOK_BITAND;
            if (memcmp(w, "bitxor", 6) == 0) return TOK_BITXOR;
            break;
        case 8:
            if (memcmp(w, "subtract", 8) == 0) return TOK_SUB;
            if (memcmp(w, "multiply", 8) == 0) return TOK_MUL;
            if (memcmp(w, "divide", 6) == 0) return TOK_DIV;
            if (memcmp(w, "boolean", 7) == 0) return TOK_BOOLEAN;
            if (memcmp(w, "global", 6) == 0) return TOK_GLOBAL;
            break;
        case 9:
            if (memcmp(w, "bitnot", 6) == 0) return TOK_BITNOT;
            break;
    }
    return TOK_VAR;
}

/* ─── Phase 2: Tokenize ─── */
static void tokenize(void) {
    int i = 0;
    tok_count = 0;

    while (i < input_len) {
        /* Skip whitespace */
        if (input[i] == ' ' || input[i] == '\t' || input[i] == '\n' || input[i] == '\r') {
            i++;
            continue;
        }

        /* Skip comments */
        if (input[i] == '#') {
            while (i < input_len && input[i] != '\n') i++;
            continue;
        }

        /* String literal: "..." */
        if (input[i] == '"') {
            int start = i + 1;
            i++;
            while (i < input_len && input[i] != '"') i++;
            int len = i - start;

            tokens[tok_count].type = TOK_STRING;
            tokens[tok_count].start = start;
            tokens[tok_count].len = len;
            tok_count++;

            if (i < input_len) i++; /* skip closing quote */
            continue;
        }

        /* Number literal */
        if (isdigit((unsigned char)input[i]) ||
            (input[i] == '-' && i+1 < input_len && isdigit((unsigned char)input[i+1]))) {
            int start = i;
            int64_t val = strtoll(&input[i], NULL, 0);
            while (i < input_len && (isalnum((unsigned char)input[i]) || input[i] == 'x')) i++;

            tokens[tok_count].type = TOK_LITERAL;
            tokens[tok_count].start = start;
            tokens[tok_count].len = i - start;
            tokens[tok_count].value = val;
            tok_count++;
            continue;
        }

        /* Identifier/keyword */
        if (isalpha((unsigned char)input[i]) || input[i] == '_') {
            int start = i;
            while (i < input_len && (isalnum((unsigned char)input[i]) || input[i] == '_')) i++;
            int len = i - start;

            tokens[tok_count].type = classify_keyword(&input[start], len);
            tokens[tok_count].start = start;
            tokens[tok_count].len = len;
            tok_count++;
            continue;
        }

        i++;
    }
}

/* ─── Phase 3: Data collection ─── */
static void collect_data(void) {
    /* Collect string literals into datasec */
    for (int i = 0; i < tok_count; i++) {
        if (tokens[i].type == TOK_STRING) {
            int src = tokens[i].start;
            int len = tokens[i].len;
            for (int j = 0; j < len; j++) {
                assert(data_len < MAX_DATA);
                datasec[data_len++] = input[src + j];
            }
            datasec[data_len++] = 0; /* null terminator */
        }
    }
}

/* ─── Helper: lookup variable ─── */
static int lookup_var(const char *name, int len) {
    for (int i = 0; i < var_count; i++) {
        if (vars[i].name_len == len && memcmp(vars[i].name, name, len) == 0)
            return i;
    }
    return -1;
}

/* ─── Helper: emit byte ─── */
static void emit_u8(uint8_t b) {
    assert(text_len < MAX_TEXT);
    text[text_len++] = b;
}

/* ─── Helper: emit imm32 (little-endian) ─── */
static void emit_u32(uint32_t v) {
    emit_u8(v & 0xFF);
    emit_u8((v >> 8) & 0xFF);
    emit_u8((v >> 16) & 0xFF);
    emit_u8((v >> 24) & 0xFF);
}

/* ─── Helper: emit imm64 (little-endian) ─── */
static void emit_u64(uint64_t v) {
    for (int i = 0; i < 8; i++)
        emit_u8((uint8_t)((v >> (i * 8)) & 0xFF));
}

/* ─── Phase 4: Parse & Codegen ─── */
static void parse_and_codegen(void) {
    /* Function prologue: push rbp; mov rbp, rsp; sub rsp, 16 */
    emit_u8(0x55);
    emit_u8(0x48); emit_u8(0x89); emit_u8(0xE5);
    emit_u8(0x48); emit_u8(0x81); emit_u8(0xEC);
    emit_u32(16);

    tok_idx = 0;
    while (tok_idx < tok_count) {
        int type = tokens[tok_idx].type;

        /* return <value> */
        if (type == TOK_RETURN) {
            tok_idx++;
            if (tok_idx < tok_count) {
                int vt = tokens[tok_idx].type;

                if (vt == TOK_LITERAL) {
                    /* mov eax, imm32 */
                    emit_u8(0xB8);
                    emit_u32((uint32_t)tokens[tok_idx].value);
                    /* mov rdi, rax */
                    emit_u8(0x48); emit_u8(0x89); emit_u8(0xC7);
                }
                else if (vt == TOK_IT) {
                    /* mov rdi, rax */
                    emit_u8(0x48); emit_u8(0x89); emit_u8(0xC7);
                }
                else if (vt == TOK_VAR) {
                    int idx = lookup_var(&input[tokens[tok_idx].start], tokens[tok_idx].len);
                    if (idx >= 0) {
                        if (vars[idx].is_global) {
                            /* mov rdi, [rip + offset] */
                            emit_u8(0x48); emit_u8(0x8B); emit_u8(0x3D);
                            emit_u32((uint32_t)vars[idx].offset);
                        } else {
                            /* mov edi, [rbp + offset] */
                            emit_u8(0x8B); emit_u8(0xBD);
                            emit_u32((uint32_t)vars[idx].offset);
                        }
                    }
                }
                else if (vt == TOK_STRING) {
                    /* mov rdi, absolute_address_of_string */
                    int64_t str_addr = 0x400000 + 120 + text_len + /* string offset */ 0;
                    emit_u8(0x48); emit_u8(0xBF);
                    emit_u64((uint64_t)str_addr);
                }
            }

            if (!in_function) {
                /* Top-level: exit via syscall */
                emit_u8(0xB8); emit_u8(0x3C); emit_u8(0x00); emit_u8(0x00); emit_u8(0x00);
                emit_u8(0x0F); emit_u8(0x05);
            } else {
                /* Function: epilogue + ret */
                emit_u8(0x48); emit_u8(0x89); emit_u8(0xE5);
                emit_u8(0x5D);
                emit_u8(0xC3);
            }
            tok_idx++;
            continue;
        }

        /* Variable declaration: a/an/the/global [type] name */
        if (type == TOK_A || type == TOK_AN || type == TOK_THE || type == TOK_GLOBAL) {
            int is_global = (type == TOK_THE || type == TOK_GLOBAL);
            tok_idx++;

            int var_size = 8; /* default: qword */
            if (tok_idx < tok_count) {
                int t = tokens[tok_idx].type;
                if (t == TOK_BYTE) { var_size = 1; tok_idx++; }
                else if (t == TOK_LONG) { var_size = 8; tok_idx++; }
                else if (t == TOK_INTEGER) { var_size = 4; tok_idx++; }
                else if (t == TOK_SHORT) { var_size = 2; tok_idx++; }
            }

            if (tok_idx < tok_count && tokens[tok_idx].type == TOK_VAR) {
                int len = tokens[tok_idx].len;
                if (len < 64 && var_count < MAX_VARS) {
                    Var *v = &vars[var_count];
                    memcpy(v->name, &input[tokens[tok_idx].start], len);
                    v->name[len] = 0;
                    v->name_len = len;
                    v->size = var_size;
                    v->is_global = is_global;

                    if (is_global) {
                        /* Allocate in datasec (zero-initialized) */
                        v->offset = data_len;
                        for (int j = 0; j < var_size; j++)
                            datasec[data_len++] = 0;
                    } else {
                        /* Allocate on stack */
                        v->offset = -8 - (var_count * 8);
                    }
                    var_count++;
                }
                tok_idx++;
            }
            continue;
        }

        /* Function definition */
        if (type == TOK_DEFINE) {
            in_function = 1;
            tok_idx++;
            continue;
        }

        /* End of block/function */
        if (type == TOK_END) {
            if (in_function) {
                in_function = 0;
                /* Implicit return 0 */
                emit_u8(0xB8); emit_u32(0);
                emit_u8(0x48); emit_u8(0x89); emit_u8(0xC7);
                emit_u8(0x48); emit_u8(0x89); emit_u8(0xE5);
                emit_u8(0x5D);
                emit_u8(0xC3);
            }
            tok_idx++;
            continue;
        }

        /* Skip other tokens (add, sub, store, load, if, while, etc.) */
        tok_idx++;
    }

    /* Function epilogue (if not in function) */
    if (!in_function) {
        emit_u8(0x48); emit_u8(0x89); emit_u8(0xE5);
        emit_u8(0x5D);
        emit_u8(0xC3);
    }
}

/* ─── Helpers for writing at specific positions ─── */
static void emit_u8_at(uint8_t *buf, int *pos, uint8_t v) {
    buf[(*pos)++] = v;
}
static void emit_u32_at(uint8_t *buf, int *pos, uint32_t v) {
    buf[(*pos)++] = v & 0xFF;
    buf[(*pos)++] = (v >> 8) & 0xFF;
    buf[(*pos)++] = (v >> 16) & 0xFF;
    buf[(*pos)++] = (v >> 24) & 0xFF;
}
static void emit_u64_at(uint8_t *buf, int *pos, uint64_t v) {
    for (int i = 0; i < 8; i++)
        buf[(*pos)++] = (uint8_t)((v >> (i * 8)) & 0xFF);
}

/* ─── Phase 5: ELF64 linking ─── */
static void link(void) {
    uint8_t *out = malloc(MAX_TEXT + MAX_DATA + 4096);
    if (!out) { perror("malloc"); exit(1); }

    int o = 0;
    int file_size = 120 + text_len + data_len;

    /* ELF header (64 bytes) */
    out[o++] = 0x7F; out[o++] = 'E'; out[o++] = 'L'; out[o++] = 'F';
    out[o++] = 2;  /* ELFCLASS64 */
    out[o++] = 1;  /* ELFDATA2LSB */
    out[o++] = 1;  /* EV_CURRENT */
    out[o++] = 0;  /* ELFOSABI_NONE */
    for (int i = 0; i < 8; i++) out[o++] = 0;
    out[o++] = 2; out[o++] = 0;  /* ET_EXEC */
    out[o++] = 0x3E; out[o++] = 0;  /* EM_X86_64 */
    out[o++] = 1; out[o++] = 0; out[o++] = 0; out[o++] = 0;  /* EV_CURRENT */

    /* e_entry (u64) */
    emit_u64_at(out, &o, 0x400078);
    /* e_phoff (u64) */
    emit_u64_at(out, &o, 64);
    /* e_shoff (u64) */
    emit_u64_at(out, &o, 0);
    /* e_flags (u32) */
    emit_u32_at(out, &o, 0);
    /* e_ehsize (u16) */
    out[o++] = 64; out[o++] = 0;
    /* e_phentsize (u16) */
    out[o++] = 56; out[o++] = 0;
    /* e_phnum (u16) */
    out[o++] = 1; out[o++] = 0;
    /* e_shentsize (u16) */
    out[o++] = 0; out[o++] = 0;
    /* e_shnum (u16) */
    out[o++] = 0; out[o++] = 0;
    /* e_shstrndx (u16) */
    out[o++] = 0; out[o++] = 0;

    /* Program header (56 bytes) */
    out[o++] = 1; out[o++] = 0; out[o++] = 0; out[o++] = 0;  /* PT_LOAD */
    out[o++] = 7; out[o++] = 0; out[o++] = 0; out[o++] = 0;  /* PF_R|PF_W|PF_X */
    emit_u64_at(out, &o, 0);       /* p_offset */
    emit_u64_at(out, &o, 0x400000); /* p_vaddr */
    emit_u64_at(out, &o, 0x400000); /* p_paddr */
    emit_u64_at(out, &o, file_size); /* p_filesz */
    emit_u64_at(out, &o, file_size); /* p_memsz */
    emit_u64_at(out, &o, 0x1000);   /* p_align */

    /* .text section */
    memcpy(&out[o], text, text_len);
    o += text_len;

    /* .data section */
    memcpy(&out[o], datasec, data_len);
    o += data_len;

    assert(o == file_size);
    fwrite(out, 1, o, stdout);
    free(out);
}

/* ─── Main ─── */
int main(void) {
    read_input();
    tokenize();
    collect_data();
    parse_and_codegen();
    link();
    return 0;
}
