/* jstar_c.c - Jasterish compiler in C
 * Mirrors the Rust implementation for comparison
 * Phases: Tokenize → Parse → Codegen → Link (ELF64)
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <ctype.h>

/* ─── Constants ─── */
#define MAX_INPUT 262144
#define MAX_TEXT 65536
#define MAX_DATA 2097152
#define MAX_TOKENS 32768
#define MAX_VARS 512
#define MAX_LABELS 512
#define MAX_FIXUPS 512

/* ─── Token types ─── */
enum TokenType {
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
    TOK_INTO = 19,
    TOK_FROM = 20,
    TOK_AT = 21,
    TOK_IT = 22,
    TOK_DEFINE = 23,
    TOK_WITH = 24,
    TOK_CALL = 25,
    TOK_SYSCALL = 26,
    TOK_BITAND = 27,
    TOK_BITOR = 28,
    TOK_BITXOR = 29,
    TOK_SHIFT = 30,
    TOK_HALT = 31,
    TOK_PUSH = 36,
    TOK_POP = 37,
    TOK_BITNOT = 38,
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
    int start;
    int len;
    int64_t value;
} Token;

/* ─── Global state ─── */
static char input[MAX_INPUT];
static int input_len = 0;

static uint8_t text[MAX_TEXT];
static int text_len = 0;

static uint8_t datasec[MAX_DATA];
static int data_len = 0;

static Token tokens[MAX_TOKENS];
static int tok_count = 0;

static int tok_start[MAX_TOKENS];
static int tok_len[MAX_TOKENS];
static int tok_type[MAX_TOKENS];
static int64_t tok_value[MAX_TOKENS];

static char var_name[MAX_VARS][64];
static int var_name_len[MAX_VARS];
static int var_offset[MAX_VARS];
static int var_count = 0;

static int vreg_next = 0;
static int vreg_total_size = 0;
static int frame_size = 0;

static int tok_idx = 0;

/* ─── Phase 1: Read input ─── */
static void read_input(void) {
    input_len = fread(input, 1, MAX_INPUT - 1, stdin);
    input[input_len] = 0;
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

        /* String literal */
        if (input[i] == '"') {
            int start = i + 1;
            i++;
            while (i < input_len && input[i] != '"') i++;
            int len = i - start;

            tok_start[tok_count] = start;
            tok_len[tok_count] = len;
            tok_type[tok_count] = TOK_STRING;
            tok_count++;

            if (i < input_len) i++; /* skip closing quote */
            continue;
        }

        /* Number literal */
        if (isdigit(input[i]) || (input[i] == '-' && isdigit(input[i+1]))) {
            int start = i;
            int64_t val = strtoll(&input[i], NULL, 0);
            while (i < input_len && (isalnum(input[i]) || input[i] == 'x')) i++;

            tok_start[tok_count] = start;
            tok_len[tok_count] = i - start;
            tok_type[tok_count] = TOK_LITERAL;
            tok_value[tok_count] = val;
            tok_count++;
            continue;
        }

        /* Identifier/keyword */
        if (isalpha(input[i]) || input[i] == '_') {
            int start = i;
            while (i < input_len && (isalnum(input[i]) || input[i] == '_')) i++;
            int len = i - start;

            /* Simple keyword matching */
            char word[64];
            memcpy(word, &input[start], len);
            word[len] = 0;

            int type = TOK_VAR;
            if (strcmp(word, "return") == 0) type = TOK_RETURN;
            else if (strcmp(word, "add") == 0) type = TOK_ADD;
            else if (strcmp(word, "subtract") == 0) type = TOK_SUB;
            else if (strcmp(word, "multiply") == 0) type = TOK_MUL;
            else if (strcmp(word, "divide") == 0) type = TOK_DIV;
            else if (strcmp(word, "store") == 0) type = TOK_STORE;
            else if (strcmp(word, "load") == 0) type = TOK_LOAD;
            else if (strcmp(word, "move") == 0) type = TOK_MOVE;
            else if (strcmp(word, "compare") == 0) type = TOK_COMPARE;
            else if (strcmp(word, "equal") == 0) type = TOK_EQUAL;
            else if (strcmp(word, "less") == 0) type = TOK_LESS;
            else if (strcmp(word, "greater") == 0) type = TOK_GREATER;
            else if (strcmp(word, "print") == 0) type = TOK_PRINT;
            else if (strcmp(word, "if") == 0) type = TOK_IF;
            else if (strcmp(word, "else") == 0) type = 14; /* TOK_ELSE */
            else if (strcmp(word, "while") == 0) type = TOK_WHILE;
            else if (strcmp(word, "end") == 0) type = TOK_END;
            else if (strcmp(word, "a") == 0 || strcmp(word, "an") == 0) type = TOK_A;
            else if (strcmp(word, "the") == 0) type = TOK_GLOBAL;
            else if (strcmp(word, "into") == 0) type = TOK_INTO;
            else if (strcmp(word, "from") == 0) type = TOK_FROM;
            else if (strcmp(word, "at") == 0) type = TOK_AT;
            else if (strcmp(word, "it") == 0 || strcmp(word, "that") == 0) type = TOK_IT;
            else if (strcmp(word, "define") == 0) type = TOK_DEFINE;
            else if (strcmp(word, "with") == 0) type = TOK_WITH;
            else if (strcmp(word, "call") == 0) type = TOK_CALL;
            else if (strcmp(word, "syscall") == 0) type = TOK_SYSCALL;
            else if (strcmp(word, "bitand") == 0) type = TOK_BITAND;
            else if (strcmp(word, "bitor") == 0) type = TOK_BITOR;
            else if (strcmp(word, "bitxor") == 0) type = TOK_BITXOR;
            else if (strcmp(word, "shift") == 0) type = TOK_SHIFT;
            else if (strcmp(word, "halt") == 0 || strcmp(word, "exit") == 0) type = TOK_HALT;
            else if (strcmp(word, "byte") == 0) type = TOK_BYTE;
            else if (strcmp(word, "long") == 0) type = TOK_LONG;
            else if (strcmp(word, "integer") == 0) type = TOK_INTEGER;
            else if (strcmp(word, "short") == 0) type = TOK_SHORT;
            else if (strcmp(word, "boolean") == 0) type = TOK_BOOLEAN;
            else if (strcmp(word, "char") == 0) type = TOK_CHAR;
            else if (strcmp(word, "global") == 0) type = TOK_GLOBAL;

            tok_start[tok_count] = start;
            tok_len[tok_count] = len;
            tok_type[tok_count] = type;
            tok_count++;
            continue;
        }

        i++;
    }
}

/* ─── Phase 2.5: Data collection ─── */
static void collect_data(void) {
    for (int i = 0; i < tok_count; i++) {
        if (tok_type[i] == TOK_STRING) {
            /* Copy string bytes to datasec */
            int src = tok_start[i];
            int len = tok_len[i];
            for (int j = 0; j < len; j++) {
                datasec[data_len++] = input[src + j];
            }
            datasec[data_len++] = 0; /* null terminator */
        }
    }
}

/* ─── Helper: lookup variable ─── */
static int lookup_var(const char *name, int len) {
    for (int i = 0; i < var_count; i++) {
        if (var_name_len[i] == len && memcmp(var_name[i], name, len) == 0) {
            return i;
        }
    }
    return -1;
}

/* ─── Helper: emit byte ─── */
static void emit_byte(uint8_t b) {
    text[text_len++] = b;
}

/* ─── Helper: emit imm32 ─── */
static void emit_imm32(int32_t val) {
    emit_byte(val & 0xFF);
    emit_byte((val >> 8) & 0xFF);
    emit_byte((val >> 16) & 0xFF);
    emit_byte((val >> 24) & 0xFF);
}

/* ─── Phase 5: Codegen ─── */
static void codegen(void) {
    /* Prologue (matching Rust) */
    emit_byte(0x55); /* push rbp */
    emit_byte(0x48); emit_byte(0x89); emit_byte(0xE5); /* mov rbp, rsp */
    /* sub rsp, 16 (stack probing, matching Rust) */
    emit_byte(0x48); emit_byte(0x81); emit_byte(0xEC);
    emit_imm32(16);

    tok_idx = 0;
    while (tok_idx < tok_count) {
        int type = tok_type[tok_idx];

        if (type == TOK_RETURN) {
            tok_idx++;
            if (tok_idx < tok_count && tok_type[tok_idx] == TOK_LITERAL) {
                int64_t val = tok_value[tok_idx];
                /* mov eax, val (matching Rust) */
                emit_byte(0xB8);
                emit_imm32((int32_t)val);
                /* mov rdi, rax (matching Rust) */
                emit_byte(0x48); emit_byte(0x89); emit_byte(0xC7);
            } else if (tok_idx < tok_count && tok_type[tok_idx] == TOK_IT) {
                /* mov rdi, rax */
                emit_byte(0x48); emit_byte(0x89); emit_byte(0xC7);
            } else if (tok_idx < tok_count && tok_type[tok_idx] == TOK_VAR) {
                int idx = lookup_var(&input[tok_start[tok_idx]], tok_len[tok_idx]);
                if (idx >= 0) {
                    /* mov edi, [rbp+offset] */
                    emit_byte(0x8B); emit_byte(0xBD);
                    emit_imm32(var_offset[idx]);
                }
            }
            if (!is_in_function) {
                /* mov eax, 60; syscall */
                emit_byte(0xB8); emit_byte(0x3C); emit_byte(0x00); emit_byte(0x00); emit_byte(0x00);
                emit_byte(0x0F); emit_byte(0x05);
            } else {
                /* mov rsp, rbp; pop rbp; ret */
                emit_byte(0x48); emit_byte(0x89); emit_byte(0xE5);
                emit_byte(0x5D);
                emit_byte(0xC3);
            }
            tok_idx++;
            continue;
        }

        if (type == TOK_ADD || type == TOK_SUB || type == TOK_MUL || type == TOK_DIV) {
            /* Simplified: just skip for now */
            tok_idx++;
            continue;
        }

        if (type == TOK_STORE) {
            /* Simplified: just skip for now */
            tok_idx++;
            continue;
        }

        if (type == TOK_LOAD) {
            /* Simplified: just skip for now */
            tok_idx++;
            continue;
        }

        if (type == TOK_IF || type == TOK_WHILE) {
            /* Simplified: just skip for now */
            tok_idx++;
            continue;
        }

        if (type == TOK_END) {
            tok_idx++;
            continue;
        }

        if (type == TOK_A || type == TOK_GLOBAL) {
            /* Variable declaration */
            tok_idx++;
            if (tok_idx < tok_count) {
                /* Skip type if present */
                int t = tok_type[tok_idx];
                if (t == TOK_BYTE || t == TOK_LONG || t == TOK_INTEGER || t == TOK_SHORT) {
                    tok_idx++;
                }
                /* Get variable name */
                if (tok_idx < tok_count && tok_type[tok_idx] == TOK_VAR) {
                    int len = tok_len[tok_idx];
                    if (len < 64) {
                        memcpy(var_name[var_count], &input[tok_start[tok_idx]], len);
                        var_name[var_count][len] = 0;
                        var_name_len[var_count] = len;
                        var_offset[var_count] = -8 - (var_count * 8);
                        var_count++;
                    }
                    tok_idx++;
                }
            }
            continue;
        }

        tok_idx++;
    }

    /* Epilogue */
    emit_byte(0x48); emit_byte(0x89); emit_byte(0xE5); /* mov rsp, rbp */
    emit_byte(0x5D); /* pop rbp */
    emit_byte(0xC3); /* ret */
}

/* ─── Phase 6: ELF linking ─── */
static void link(void) {
    uint8_t output[MAX_TEXT + MAX_DATA + 4096];
    int out_len = 0;
    int file_size = 120 + text_len + data_len;

    /* ELF header (64 bytes) - matching Rust exactly */
    output[out_len++] = 0x7F; output[out_len++] = 'E'; output[out_len++] = 'L'; output[out_len++] = 'F';
    output[out_len++] = 2; /* 64-bit */
    output[out_len++] = 1; /* LE */
    output[out_len++] = 1; /* Version */
    output[out_len++] = 0; /* OS/ABI */
    for (int i = 0; i < 8; i++) output[out_len++] = 0; /* padding */
    output[out_len++] = 2; output[out_len++] = 0; /* ET_EXEC */
    output[out_len++] = 0x3E; output[out_len++] = 0; /* EM_X86_64 */
    output[out_len++] = 1; output[out_len++] = 0; output[out_len++] = 0; output[out_len++] = 0; /* EV_CURRENT */

    /* Entry point = 0x400078 (little-endian u64) */
    int64_t entry = 0x400078;
    for (int i = 0; i < 8; i++) output[out_len++] = (uint8_t)((entry >> (i * 8)) & 0xFF);

    /* Program header offset = 64 (u64) */
    int64_t phoff = 64;
    for (int i = 0; i < 8; i++) output[out_len++] = (uint8_t)((phoff >> (i * 8)) & 0xFF);

    /* Section header offset = 0 (u64) */
    int64_t shoff = 0;
    for (int i = 0; i < 8; i++) output[out_len++] = (uint8_t)((shoff >> (i * 8)) & 0xFF);

    /* Flags (u32) */
    output[out_len++] = 0; output[out_len++] = 0; output[out_len++] = 0; output[out_len++] = 0;

    /* ELF header size = 64 (u16) */
    output[out_len++] = 64; output[out_len++] = 0;

    /* Program header size = 56 (u16) */
    output[out_len++] = 56; output[out_len++] = 0;

    /* Number of program headers = 1 (u16) */
    output[out_len++] = 1; output[out_len++] = 0;

    /* Section header size = 0 (u16) */
    output[out_len++] = 0; output[out_len++] = 0;

    /* Number of section headers = 0 (u16) */
    output[out_len++] = 0; output[out_len++] = 0;

    /* Section name string table index = 0 (u16) */
    output[out_len++] = 0; output[out_len++] = 0;

    /* Program header (56 bytes) */
    output[out_len++] = 1; output[out_len++] = 0; output[out_len++] = 0; output[out_len++] = 0; /* PT_LOAD */
    output[out_len++] = 7; output[out_len++] = 0; output[out_len++] = 0; output[out_len++] = 0; /* PF_R|PF_W|PF_X (matching Rust) */

    /* Offset = 0 (u64) */
    for (int i = 0; i < 8; i++) output[out_len++] = 0;

    /* Vaddr = 0x400000 (u64) */
    int64_t vaddr = 0x400000;
    for (int i = 0; i < 8; i++) output[out_len++] = (uint8_t)((vaddr >> (i * 8)) & 0xFF);

    /* Paddr = 0x400000 (u64) */
    for (int i = 0; i < 8; i++) output[out_len++] = (uint8_t)((vaddr >> (i * 8)) & 0xFF);

    /* File size (u64) */
    for (int i = 0; i < 8; i++) output[out_len++] = (uint8_t)((file_size >> (i * 8)) & 0xFF);

    /* Mem size (u64) */
    for (int i = 0; i < 8; i++) output[out_len++] = (uint8_t)((file_size >> (i * 8)) & 0xFF);

    /* Align = 0x1000 (u64) */
    output[out_len++] = 0; output[out_len++] = 0x10; output[out_len++] = 0; output[out_len++] = 0;
    for (int i = 0; i < 4; i++) output[out_len++] = 0;

    /* .text section */
    memcpy(&output[out_len], text, text_len);
    out_len += text_len;

    /* .data section */
    memcpy(&output[out_len], datasec, data_len);
    out_len += data_len;

    fwrite(output, 1, out_len, stdout);
}

/* ─── Main ─── */
int main(void) {
    read_input();
    tokenize();
    collect_data();
    codegen();
    link();
    return 0;
}
