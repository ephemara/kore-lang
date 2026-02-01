// KORE Runtime V2 - Minimal C Implementation + Shims
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

// Helper
char* kore_strdup(const char* s) {
    size_t len = strlen(s);
    char* d = malloc(len + 1);
    strcpy(d, s);
    return d;
}

// Print functions
void kore_print_i64(int64_t n) { printf("%lld", n); }
void kore_print_str(const char* s) { printf("%s", s); }
void kore_println_str(const char* s) { printf("%s\n", s); }
void kore_print_newline(void) { printf("\n"); }

// String functions
char* kore_str_concat(const char* a, const char* b) {
    size_t len_a = strlen(a);
    size_t len_b = strlen(b);
    char* result = malloc(len_a + len_b + 1);
    strcpy(result, a);
    strcat(result, b);
    return result;
}

int64_t kore_str_len(const char* s) { return strlen(s); }
int64_t kore_str_eq(const char* a, const char* b) { return strcmp(a, b) == 0 ? 1 : 0; }

char* kore_to_string(int64_t n) {
    char* buf = malloc(32);
    sprintf(buf, "%lld", n);
    return buf;
}

int64_t kore_to_int(const char* s) { return atoi(s); }

// Array functions (simple growable array using realloc)
typedef struct { int64_t* data; int64_t len; int64_t cap; } KoreArray;

int64_t kore_array_new(void) {
    KoreArray* arr = malloc(sizeof(KoreArray));
    arr->data = malloc(8 * sizeof(int64_t));
    arr->len = 0;
    arr->cap = 8;
    return (int64_t)arr;
}

void kore_array_push(int64_t arr_ptr, int64_t value) {
    KoreArray* arr = (KoreArray*)arr_ptr;
    if (arr->len >= arr->cap) {
        arr->cap *= 2;
        arr->data = realloc(arr->data, arr->cap * sizeof(int64_t));
    }
    arr->data[arr->len++] = value;
}

int64_t kore_array_get(int64_t arr_ptr, int64_t index) {
    KoreArray* arr = (KoreArray*)arr_ptr;
    if (index < 0 || index >= arr->len) return 0;
    return arr->data[index];
}

int64_t kore_array_len(int64_t arr_ptr) {
    KoreArray* arr = (KoreArray*)arr_ptr;
    return arr->len;
}

int64_t kore_array_pop(int64_t arr_ptr) {
    KoreArray* arr = (KoreArray*)arr_ptr;
    if (arr->len > 0) {
        arr->len--;
        return arr->data[arr->len];
    }
    return 0;
}

void kore_array_set(int64_t arr_ptr, int64_t index, int64_t value) {
    KoreArray* arr = (KoreArray*)arr_ptr;
    if (index >= 0 && index < arr->len) {
        arr->data[index] = value;
    }
}

// Map functions (simple linear search)
typedef struct { char** keys; int64_t* values; int64_t len; int64_t cap; } KoreMap;

int64_t Map_new(void) {
    KoreMap* m = malloc(sizeof(KoreMap));
    m->keys = malloc(16 * sizeof(char*));
    m->values = malloc(16 * sizeof(int64_t));
    m->len = 0;
    m->cap = 16;
    return (int64_t)m;
}

void kore_map_set(int64_t map_ptr, const char* key, int64_t value) {
    KoreMap* m = (KoreMap*)map_ptr;
    // Check if key exists
    for (int64_t i = 0; i < m->len; i++) {
        if (strcmp(m->keys[i], key) == 0) {
            m->values[i] = value;
            return;
        }
    }
    // Add new
    if (m->len >= m->cap) {
        m->cap *= 2;
        m->keys = realloc(m->keys, m->cap * sizeof(char*));
        m->values = realloc(m->values, m->cap * sizeof(int64_t));
    }
    m->keys[m->len] = kore_strdup(key);
    m->values[m->len] = value;
    m->len++;
}

int64_t kore_map_get(int64_t map_ptr, const char* key) {
    KoreMap* m = (KoreMap*)map_ptr;
    for (int64_t i = 0; i < m->len; i++) {
        if (strcmp(m->keys[i], key) == 0) {
            return m->values[i];
        }
    }
    return 0;
}

int64_t kore_contains_key(int64_t map_ptr, const char* key) {
    KoreMap* m = (KoreMap*)map_ptr;
    for (int64_t i = 0; i < m->len; i++) {
        if (strcmp(m->keys[i], key) == 0) {
            return 1;
        }
    }
    return 0;
}

// File I/O
char* kore_file_read(const char* path) {
    // printf("DEBUG: kore_file_read called with ptr %p\n", path); fflush(stdout);
    if (!path) {
        // printf("DEBUG: path is NULL\n"); fflush(stdout);
        return NULL;
    }
    // printf("DEBUG: kore_file_read opening '%s'\n", path); fflush(stdout);
    FILE* f = fopen(path, "rb");
    if (!f) {
        // printf("DEBUG: fopen failed for '%s'\n", path); fflush(stdout);
        return NULL;
    }
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    char* buf = malloc(size + 1);
    fread(buf, 1, size, f);
    buf[size] = '\0';
    fclose(f);
    // printf("DEBUG: kore_file_read success, size=%ld\n", size); fflush(stdout);
    return buf;
}

int64_t kore_file_write(const char* path, const char* content) {
    FILE* f = fopen(path, "wb");
    if (!f) return 0;
    fputs(content, f);
    fclose(f);
    return 1;
}

// Memory
void* kore_alloc(int64_t size) { return malloc(size); }
void kore_free(void* ptr) { free(ptr); }

// Panic
void kore_panic(const char* msg) {
    fprintf(stderr, "PANIC: %s\n", msg);
    exit(1);
}

// Misc / Missing
int64_t kore_add_op(int64_t a, int64_t b) { return a + b; }
int64_t kore_none() { return 0; }
int64_t kore_some(int64_t val) { return val; } // Naive option

// Variant access (Assuming layout: tag(i64), payload(i8*), name(i8*))
// Pointer points to struct.
// Field 0 at offset 0, Field 1 at offset 8, Field 2 at offset 16.
int64_t kore_variant_of(int64_t ptr_val) {
    char* ptr = (char*)ptr_val;
    char** name_ptr = (char**)(ptr + 16);
    return (int64_t)(*name_ptr);
}

int64_t kore_variant_field(int64_t ptr_val, int64_t idx) {
    char* ptr = (char*)ptr_val;
    int64_t* payload_ptr = (int64_t*)(ptr + 8);
    int64_t* tuple = (int64_t*)(*payload_ptr); // Payload is pointer to tuple
    if (!tuple) return 0;
    return tuple[idx];
}

// Missing Shims
char* read_file(const char* path) { return kore_file_read(path); }
int64_t write_file(const char* path, const char* content) { return kore_file_write(path, content); }

int64_t kore_contains(int64_t col_ptr, int64_t val) {
    // Assume Array of Strings (korec usage)
    KoreArray* arr = (KoreArray*)col_ptr;
    for (int i=0; i<arr->len; i++) {
        if (kore_str_eq((char*)arr->data[i], (char*)val)) return 1;
    }
    return 0;
}

int64_t kore_split(int64_t str_ptr, int64_t sep_ptr) {
    char* s = (char*)str_ptr;
    char* sep = (char*)sep_ptr;
    // printf("DEBUG: kore_split s='%s' sep='%s'\n", s ? s : "NULL", sep ? sep : "NULL");
    
    int64_t arr = kore_array_new();
    
    if (strlen(sep) == 0) {
        size_t len = strlen(s);
        for (size_t i = 0; i < len; i++) {
            char* c_str = malloc(2);
            c_str[0] = s[i];
            c_str[1] = 0;
            kore_array_push(arr, (int64_t)c_str);
        }
        return arr;
    }
    
    char* current = s;
    char* next_match = strstr(current, sep);
    size_t sep_len = strlen(sep);
    
    while (next_match) {
        size_t segment_len = next_match - current;
        char* segment = malloc(segment_len + 1);
        strncpy(segment, current, segment_len);
        segment[segment_len] = 0;
        kore_array_push(arr, (int64_t)segment);
        
        current = next_match + sep_len;
        next_match = strstr(current, sep);
    }
    kore_array_push(arr, (int64_t)kore_strdup(current));
    
    return arr;
}

char* kore_join(int64_t arr_ptr, int64_t sep_ptr) {
    KoreArray* arr = (KoreArray*)arr_ptr;
    char* sep = (char*)sep_ptr;
    
    if (arr->len == 0) return kore_strdup("");
    
    size_t total_len = 0;
    size_t sep_len = strlen(sep);
    for (int i=0; i<arr->len; i++) {
        total_len += strlen((char*)arr->data[i]);
        if (i < arr->len - 1) total_len += sep_len;
    }
    
    char* res = malloc(total_len + 1);
    res[0] = 0;
    
    for (int i=0; i<arr->len; i++) {
        strcat(res, (char*)arr->data[i]);
        if (i < arr->len - 1) strcat(res, sep);
    }
    return res;
}

char* kore_substring(int64_t str_ptr, int64_t start, int64_t end) {
    char* s = (char*)str_ptr;
    int64_t len = strlen(s);
    if (start < 0) start = 0;
    if (end > len) end = len;
    if (start >= end) return kore_strdup("");
    
    int64_t new_len = end - start;
    char* sub = malloc(new_len + 1);
    memcpy(sub, s + start, new_len);
    sub[new_len] = 0;
    return sub;
}

double kore_to_float(int64_t val) { return (double)val; } // Garbage

int64_t kore_range(int64_t start, int64_t end) {
    int64_t arr = kore_array_new();
    for (int64_t i=start; i<end; i++) {
        kore_array_push(arr, i);
    }
    return arr;
}

int64_t kore_ord(int64_t str_ptr) {
    char* s = (char*)str_ptr;
    if (!s || !*s) return 0;
    return (int64_t)(unsigned char)s[0];
}

char* kore_chr(int64_t n) {
    char* s = malloc(2);
    s[0] = (char)n;
    s[1] = 0;
    return s;
}

// Entry Point & Args
static int g_argc = 0;
static char** g_argv = NULL;

int64_t args(void) {
    int64_t arr = kore_array_new();
    for (int i = 0; i < g_argc; i++) {
        kore_array_push(arr, (int64_t)g_argv[i]);
    }
    return arr;
}

extern int64_t main_kore(void);

int main(int argc, char** argv) {
    g_argc = argc;
    g_argv = argv;
    return (int)main_kore();
}
